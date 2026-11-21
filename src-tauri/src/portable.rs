use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

const APP_FOLDER: &str = "ShrinkWise";
static APP_DIR: OnceLock<PathBuf> = OnceLock::new();

fn is_dir_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".sw-probe-{}", std::process::id()));
    if fs::write(&probe, b"").is_ok() {
        let _ = fs::remove_file(&probe);
        return true;
    }
    false
}

fn resolve_app_dir() -> PathBuf {
    #[cfg(debug_assertions)]
    {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        if let Some(parent) = manifest.parent() {
            return parent.to_path_buf();
        }
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    if is_dir_writable(&exe_dir) {
        return exe_dir;
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA")
        && !local.trim().is_empty()
    {
        return PathBuf::from(local).join(APP_FOLDER);
    }
    std::env::temp_dir().join(APP_FOLDER)
}

/// Taşınabilir veya kurulu çalışma kökü.
///
/// * Geliştirme sırasında proje kökü.
/// * Paketlenmiş/kurulu sürümde exe'nin bulunduğu klasör (yazılabilirse),
///   aksi halde (ör. `Program Files` kurulumu) `%LOCALAPPDATA%\ShrinkWise`.
/// * `SHRINKWISE_HOME` ortam değişkeni her zaman önceliklidir.
pub fn app_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("SHRINKWISE_HOME")
        && !custom.trim().is_empty()
    {
        return PathBuf::from(custom);
    }
    APP_DIR.get_or_init(resolve_app_dir).clone()
}

pub fn settings_path() -> PathBuf {
    app_dir().join("settings.json")
}

pub fn tools_dir() -> PathBuf {
    app_dir().join("tools").join("bin")
}

pub fn logs_dir() -> PathBuf {
    app_dir().join("logs")
}

/// Geçici dosyalar uygulamanın yanında tutulur (taşınabilirlik).
/// Klasör oluşturulamazsa sistem geçici klasörüne düşer.
pub fn temp_dir() -> PathBuf {
    let preferred = app_dir().join("temp");
    if fs::create_dir_all(&preferred).is_ok() {
        return preferred;
    }
    let fallback = std::env::temp_dir().join(APP_FOLDER);
    let _ = fs::create_dir_all(&fallback);
    fallback
}

pub fn ensure_dirs() -> Result<()> {
    for dir in [tools_dir(), logs_dir(), temp_dir()] {
        fs::create_dir_all(&dir)
            .with_context(|| format!("could not create folder: {}", dir.display()))?;
    }
    cleanup_stale_temp(&temp_dir());
    Ok(())
}

pub fn history_path() -> PathBuf {
    app_dir().join("history.jsonl")
}

/// Appends one JSON line to the job history file. Failures are ignored: history
/// is a convenience, never a reason to fail a compression job.
pub fn append_history(line: &str) {
    use std::io::Write;
    let path = history_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{line}");
    }
}

/// Temporary files left behind by a crash are removed after a day. Anything
/// younger may belong to a still running instance.
fn cleanup_stale_temp(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let now = std::time::SystemTime::now();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let ours = [".sw-", ".swcopy-", ".swbak-", ".swtext-"]
            .iter()
            .any(|prefix| name.starts_with(prefix));
        if !ours {
            continue;
        }
        let stale = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age.as_secs() > 86_400);
        if stale {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Free bytes on the volume that contains `path`, if the platform reports it.
pub fn available_space(path: &Path) -> Option<u64> {
    fn normalized(path: &Path) -> String {
        let text = path.to_string_lossy().replace('/', "\\");
        let text = text.strip_prefix("\\\\?\\").unwrap_or(&text);
        text.to_ascii_lowercase()
    }

    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let target = normalized(&target);
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut best: Option<(usize, u64)> = None;
    for disk in disks.list() {
        let mount = normalized(disk.mount_point());
        if mount.is_empty() || !target.starts_with(&mount) {
            continue;
        }
        let space = disk.available_space();
        if best.is_none_or(|(len, _)| mount.len() > len) {
            best = Some((mount.len(), space));
        }
    }
    best.map(|(_, space)| space)
}

/// Aynı diskte atomik yazma: önce `.tmp` dosyası, sonra rename.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("file"),
        std::process::id()
    ));
    fs::write(&tmp, bytes)
        .with_context(|| format!("could not write temporary file: {}", tmp.display()))?;
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(_) => {
            // Windows: hedef varsa rename basarisiz olabilir.
            let _ = fs::remove_file(path);
            fs::rename(&tmp, path)
                .with_context(|| format!("could not write file: {}", path.display()))?;
            Ok(())
        }
    }
}

#[cfg(windows)]
pub fn new_command(path: impl AsRef<std::ffi::OsStr>) -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut cmd = Command::new(path);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[cfg(not(windows))]
pub fn new_command(path: impl AsRef<std::ffi::OsStr>) -> Command {
    Command::new(path)
}

pub struct Captured {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

/// Alt süreci zaman aşımıyla çalıştırır; çıktıyı toplar.
pub fn run_capture(
    program: impl AsRef<std::ffi::OsStr>,
    args: &[String],
    timeout: Duration,
) -> Result<Captured> {
    let mut child = new_command(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("could not start subprocess")?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let out_thread = std::thread::spawn(move || read_all(stdout));
    let err_thread = std::thread::spawn(move || read_all(stderr));

    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait().context("could not read process status")? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("operation timed out ({} s)", timeout.as_secs());
        }
        std::thread::sleep(Duration::from_millis(25));
    };

    let stdout = out_thread.join().unwrap_or_default();
    let stderr = err_thread.join().unwrap_or_default();
    Ok(Captured {
        status,
        stdout,
        stderr,
    })
}

fn read_all<R: Read + Send + 'static>(reader: Option<R>) -> String {
    let mut buf = Vec::new();
    if let Some(mut reader) = reader {
        let _ = reader.read_to_end(&mut buf);
    }
    String::from_utf8_lossy(&buf).to_string()
}

/// Çalışan ffmpeg alt süreçlerini izlemek için kullanılır.
pub fn kill_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

// ---------------------------------------------------------------------------
//  Windows Job Object: ana uygulama kapanınca (çökse bile) alt süreçler ölür.
// ---------------------------------------------------------------------------
static JOB_HANDLE: OnceLock<isize> = OnceLock::new();

#[cfg(windows)]
pub fn install_job_object() -> bool {
    use std::mem::size_of;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    if JOB_HANDLE.get().is_some() {
        return true;
    }
    // SAFETY: Win32 Job Object FFI calls use valid handles and a zero-initialized limit struct.
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return false;
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ok = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const std::ffi::c_void,
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        if ok == 0 || AssignProcessToJobObject(job, GetCurrentProcess()) == 0 {
            let _ = CloseHandle(job);
            return false;
        }
        let _ = JOB_HANDLE.set(job as isize);
        true
    }
}

#[cfg(windows)]
fn is_valid_webview2_dir(dir: &Path) -> bool {
    dir.join("msedgewebview2.exe").is_file()
        && dir
            .join("EBWebView")
            .join("x64")
            .join("EmbeddedBrowserWebView.dll")
            .is_file()
}

#[cfg(windows)]
fn find_webview2_runtime_dir() -> Option<PathBuf> {
    let bases = [
        r"C:\Program Files (x86)\Microsoft\EdgeWebView\Application",
        r"C:\Program Files (x86)\Microsoft\EdgeCore",
        r"C:\Program Files (x86)\Microsoft\Edge\Application",
        r"C:\Program Files\Microsoft\EdgeWebView\Application",
        r"C:\Program Files\Microsoft\EdgeCore",
        r"C:\Program Files\Microsoft\Edge\Application",
    ];
    for base in bases {
        let Ok(entries) = fs::read_dir(base) else {
            continue;
        };
        let mut candidates: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir() && is_valid_webview2_dir(p))
            .collect();
        candidates.sort();
        if let Some(best) = candidates.pop() {
            return Some(best);
        }
    }
    let inbox = PathBuf::from(r"C:\Windows\System32\Microsoft-Edge-WebView");
    if is_valid_webview2_dir(&inbox) {
        return Some(inbox);
    }
    if let Ok(entries) = fs::read_dir(r"C:\Windows\WinSxS") {
        let mut candidates: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("amd64_microsoft-edge-webview_"))
                    && is_valid_webview2_dir(p)
            })
            .collect();
        candidates.sort();
        if let Some(best) = candidates.pop() {
            return Some(best);
        }
    }
    None
}

#[cfg(windows)]
const WEBVIEW2_CLIENT_GUID: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";

#[cfg(windows)]
fn has_valid_webview2_client_state() -> bool {
    let keys = [
        format!(
            r"HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\ClientState\{WEBVIEW2_CLIENT_GUID}"
        ),
        format!(r"HKLM\SOFTWARE\Microsoft\EdgeUpdate\ClientState\{WEBVIEW2_CLIENT_GUID}"),
        format!(r"HKCU\Software\Microsoft\EdgeUpdate\ClientState\{WEBVIEW2_CLIENT_GUID}"),
    ];
    for key in keys {
        let Ok(out) = new_command("reg.exe")
            .args(["query", &key, "/v", "EBWebView"])
            .output()
        else {
            continue;
        };
        if !out.status.success() {
            continue;
        }
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if let Some((_, path_str)) = line.split_once("REG_SZ") {
                let candidate = PathBuf::from(path_str.trim());
                let is_versioned = candidate
                    .file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.chars().next().is_some_and(|c| c.is_ascii_digit()));
                if is_versioned && is_valid_webview2_dir(&candidate) {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(windows)]
pub fn configure_webview2() {
    if std::env::var_os("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER").is_some() {
        return;
    }
    let in_sandbox = std::env::var("USERNAME")
        .map(|u| u.eq_ignore_ascii_case("WDAGUtilityAccount"))
        .unwrap_or(false);
    if !in_sandbox && has_valid_webview2_client_state() {
        return;
    }
    if let Some(runtime_dir) = find_webview2_runtime_dir() {
        // SAFETY: Called on the main thread at startup before any worker threads are spawned.
        unsafe {
            std::env::set_var("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER", runtime_dir);
        }
    }
}

#[cfg(not(windows))]
pub fn configure_webview2() {}

#[cfg(not(windows))]
pub fn install_job_object() -> bool {
    false
}

fn is_installed() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|dir| dir.join("uninstall.exe").is_file()))
        .unwrap_or(false)
}

/// Yalnızca test/tanı için.
pub fn app_paths() -> serde_json::Value {
    let mode = if cfg!(debug_assertions) {
        "development"
    } else if is_installed() {
        "installed"
    } else {
        "portable"
    };
    serde_json::json!({
        "appDir": app_dir().display().to_string(),
        "settings": settings_path().display().to_string(),
        "tools": tools_dir().display().to_string(),
        "temp": temp_dir().display().to_string(),
        "logs": logs_dir().display().to_string(),
        "mode": mode,
    })
}
