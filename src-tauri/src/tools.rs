use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

use crate::portable;
use crate::settings::Settings;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub id: String,
    pub name: String,
    pub path: Option<String>,
    pub version: Option<String>,
    pub source: String,
    pub available: bool,
    pub required: bool,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ToolsStatus {
    pub ffmpeg: ToolInfo,
    pub ffprobe: ToolInfo,
    pub seven_zip: ToolInfo,
    pub realesrgan: ToolInfo,
    pub waifu2x: ToolInfo,
    pub ghostscript: ToolInfo,
    pub qpdf: ToolInfo,
    pub tools_dir: String,
    pub encoders: Vec<String>,
    pub portable: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CodecSupport {
    pub h264: bool,
    pub hevc: bool,
    pub av1: bool,
}

impl CodecSupport {
    pub fn any(&self) -> bool {
        self.h264 || self.hevc || self.av1
    }
    pub fn labels(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.h264 {
            out.push("H.264");
        }
        if self.hevc {
            out.push("HEVC");
        }
        if self.av1 {
            out.push("AV1");
        }
        out
    }
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EncoderSupport {
    pub vendor: String,
    pub nvenc: CodecSupport,
    pub amf: CodecSupport,
    pub qsv: CodecSupport,
    pub recommended: String,
    pub note: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolProgress {
    pub id: String,
    pub phase: String,
    pub downloaded: u64,
    pub total: u64,
    pub percent: f64,
    pub message: String,
}

const FFMPEG_WIN_URL: &str = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";
const SEVENZIP_URL: &str = "https://www.7-zip.org/a/7zr.exe";
const REALESRGAN_URL: &str = "https://github.com/xinntao/Real-ESRGAN/releases/download/v0.2.5.0/realesrgan-ncnn-vulkan-20220424-windows.zip";
const WAIFU2X_URL: &str = "https://github.com/nihui/waifu2x-ncnn-vulkan/releases/download/20250915/waifu2x-ncnn-vulkan-20250915-windows.zip";
const WAIFU2X_SHA256: &str = "7425be94b94e4c8f37a1e433ac0e0100c43790e2c37418f4b65d8235adfbdc87";
const QPDF_URL: &str =
    "https://github.com/qpdf/qpdf/releases/download/v12.4.2/qpdf-12.4.2-msvc64.zip";
const QPDF_SHA256: &str = "db87077e683630c1217e0e8f9a20a9749d952ab676e881c3689187763a5de25d";
const GHOSTSCRIPT_URL: &str = "https://github.com/ArtifexSoftware/ghostpdl-downloads/releases/download/gs10080/gs10080w64.exe";
const GHOSTSCRIPT_SHA256: &str = "52a91b8bf09298788d7a57b9206127026c23eacd75405f0a131e26dc381dce50";

fn exe_name(base: &str) -> String {
    if cfg!(windows) {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

fn extra_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    #[cfg(windows)]
    {
        for dir in [
            r"C:\Program Files\7-Zip",
            r"C:\Program Files (x86)\7-Zip",
            r"C:\ffmpeg\bin",
            r"C:\Program Files\ffmpeg\bin",
            r"C:\Program Files (x86)\ffmpeg\bin",
            r"C:\Program Files\qpdf\bin",
            r"C:\Program Files (x86)\qpdf\bin",
            r"C:\ProgramData\chocolatey\bin",
        ] {
            dirs.push(PathBuf::from(dir));
        }
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let local_path = PathBuf::from(&local);
            dirs.push(local_path.join("Programs").join("7-Zip"));
            dirs.push(local_path.join("Microsoft").join("WinGet").join("Links"));
        }
        if let Ok(profile) = std::env::var("USERPROFILE") {
            dirs.push(PathBuf::from(profile).join("scoop").join("shims"));
        }
        for root in [r"C:\Program Files\gs", r"C:\Program Files (x86)\gs"] {
            if let Ok(entries) = fs::read_dir(root) {
                for entry in entries.flatten() {
                    dirs.push(entry.path().join("bin"));
                }
            }
        }
        for root in [r"C:\Program Files", r"C:\Program Files (x86)"] {
            if let Ok(entries) = fs::read_dir(root) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_lowercase();
                    if name.starts_with("qpdf") {
                        dirs.push(entry.path().join("bin"));
                    }
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        dirs.push(PathBuf::from("/opt/homebrew/bin"));
        dirs.push(PathBuf::from("/usr/local/bin"));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        dirs.push(PathBuf::from("/usr/local/bin"));
        dirs.push(PathBuf::from("/usr/bin"));
    }
    dirs
}

fn find_tool(names: &[&str], custom: &str, extra: &[PathBuf]) -> Option<(PathBuf, String)> {
    let custom = custom.trim();
    if !custom.is_empty() {
        let path = PathBuf::from(custom);
        if path.is_file() {
            return Some((path, "custom".into()));
        }
    }

    let portable_dir = portable::tools_dir();
    let portable_dirs = [
        portable_dir.clone(),
        portable_dir.join("qpdf"),
        portable_dir.join("ghostscript").join("bin"),
        portable_dir.join("ghostscript"),
    ];
    let system_dirs: Vec<PathBuf> = portable_dir
        .parent()
        .map(|p| p.to_path_buf())
        .into_iter()
        .chain(extra.iter().cloned())
        .chain(extra_dirs())
        .collect();

    // Check by binary name priority so full `7z.exe` (even on system) is preferred
    // over reduced standalone `7zr.exe`.
    for name in names {
        for pdir in &portable_dirs {
            let candidate = pdir.join(exe_name(name));
            if candidate.is_file() {
                return Some((candidate, "portable".into()));
            }
        }
        if let Ok(path) = which::which(name) {
            return Some((path, "system".into()));
        }
        for dir in &system_dirs {
            let sys_candidate = dir.join(exe_name(name));
            if sys_candidate.is_file() {
                return Some((sys_candidate, "system".into()));
            }
        }
    }
    None
}

fn find_upscaler(exe: &str) -> Option<(PathBuf, String)> {
    let portable_dir = portable::tools_dir();
    let direct = portable_dir.join(exe_name(exe));
    if direct.is_file() {
        return Some((direct, "portable".into()));
    }
    if let Ok(entries) = fs::read_dir(&portable_dir) {
        for entry in entries.flatten() {
            let candidate = entry.path().join(exe_name(exe));
            if candidate.is_file() {
                return Some((candidate, "portable".into()));
            }
        }
    }
    which::which(exe).ok().map(|path| (path, "system".into()))
}

fn tool_info(
    id: &str,
    name: &str,
    found: Option<(PathBuf, String)>,
    version: Option<String>,
) -> ToolInfo {
    match found {
        Some((path, source)) => ToolInfo {
            id: id.into(),
            name: name.into(),
            path: Some(path.display().to_string()),
            version,
            source,
            available: true,
            required: id == "ffmpeg" || id == "ffprobe",
            note: String::new(),
        },
        None => ToolInfo {
            id: id.into(),
            name: name.into(),
            path: None,
            version: None,
            source: "missing".into(),
            available: false,
            required: id == "ffmpeg" || id == "ffprobe",
            note: if id == "ffmpeg" {
                "Required for video, audio and GIF processing".into()
            } else {
                String::new()
            },
        },
    }
}

fn seven_zip_version(path: &Path) -> Option<String> {
    let out = portable::run_capture(path, &[], Duration::from_secs(5)).ok()?;
    let text = if out.stdout.trim().is_empty() {
        out.stderr
    } else {
        out.stdout
    };
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("7-Zip") {
            let head = trimmed.split(':').next().unwrap_or(trimmed).trim();
            return Some(head.to_string());
        }
    }
    None
}

pub fn probe_single(settings: &Settings, extra: &[PathBuf], id: &str) -> Result<ToolInfo> {
    let info = match id {
        "ffmpeg" => {
            let found = find_tool(&["ffmpeg"], &settings.ffmpeg_path, extra);
            let ver = found
                .as_ref()
                .and_then(|(p, _)| version_of(p, &["-hide_banner", "-version"], "ffmpeg version"));
            tool_info("ffmpeg", "FFmpeg", found, ver)
        }
        "ffprobe" => {
            let found = find_tool(&["ffprobe"], &settings.ffprobe_path, extra);
            let ver = found
                .as_ref()
                .and_then(|(p, _)| version_of(p, &["-hide_banner", "-version"], "ffprobe version"));
            tool_info("ffprobe", "FFprobe", found, ver)
        }
        "seven_zip" | "sevenZip" => {
            let found = find_tool(&["7z", "7za", "7zr"], &settings.seven_zip_path, extra);
            let ver = found.as_ref().and_then(|(p, _)| seven_zip_version(p));
            tool_info("seven_zip", "7-Zip", found, ver)
        }
        "realesrgan" => {
            let found = find_upscaler("realesrgan-ncnn-vulkan");
            let ver = found.as_ref().map(|_| "v0.2.5.0 (ncnn-vulkan)".to_string());
            tool_info("realesrgan", "Real-ESRGAN", found, ver)
        }
        "waifu2x" => {
            let found = find_upscaler("waifu2x-ncnn-vulkan");
            let ver = found.as_ref().map(|_| "20250915 (ncnn-vulkan)".to_string());
            tool_info("waifu2x", "Waifu2x", found, ver)
        }
        "ghostscript" => {
            let found = find_tool(
                &["gswin64c", "gswin32c", "gs"],
                &settings.ghostscript_path,
                extra,
            );
            let ver = found
                .as_ref()
                .and_then(|(p, _)| version_of(p, &["--version"], ""));
            tool_info("ghostscript", "Ghostscript", found, ver)
        }
        "qpdf" => {
            let found = find_tool(&["qpdf"], &settings.qpdf_path, extra);
            let ver = found
                .as_ref()
                .and_then(|(p, _)| version_of(p, &["--version"], "qpdf version"));
            tool_info("qpdf", "qpdf", found, ver)
        }
        other => bail!("unknown tool: {other}"),
    };
    Ok(info)
}

pub fn probe(settings: &Settings, extra: &[PathBuf], with_encoders: bool) -> ToolsStatus {
    let get = |id: &str| probe_single(settings, extra, id).unwrap_or_default();
    let ffmpeg = get("ffmpeg");
    let encoders = if with_encoders {
        ffmpeg
            .path
            .as_deref()
            .map(Path::new)
            .map(probe_encoders)
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let tools_dir = portable::tools_dir();
    let portable_mode = ffmpeg
        .path
        .as_deref()
        .map(|p| Path::new(p).starts_with(&tools_dir))
        .unwrap_or(false);

    ToolsStatus {
        ffmpeg,
        ffprobe: get("ffprobe"),
        seven_zip: get("seven_zip"),
        realesrgan: get("realesrgan"),
        waifu2x: get("waifu2x"),
        ghostscript: get("ghostscript"),
        qpdf: get("qpdf"),
        tools_dir: tools_dir.display().to_string(),
        encoders,
        portable: portable_mode,
    }
}

fn version_of(path: &Path, args: &[&str], prefix: &str) -> Option<String> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let out = portable::run_capture(path, &args, Duration::from_secs(10)).ok()?;
    let text = if out.stdout.trim().is_empty() {
        out.stderr
    } else {
        out.stdout
    };
    for line in text.lines() {
        if let Some(rest) = line.trim().strip_prefix(prefix) {
            return Some(rest.split_whitespace().next().unwrap_or("").into());
        }
    }
    None
}

pub fn probe_encoders(ffmpeg: &Path) -> Vec<String> {
    let args = vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-encoders".to_string(),
    ];
    let Ok(out) = portable::run_capture(ffmpeg, &args, Duration::from_secs(20)) else {
        return Vec::new();
    };
    let mut encoders = HashSet::new();
    for line in out.stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let flags = parts[0];
            if flags.len() == 6 && flags.chars().all(|c| "VASD.".contains(c)) {
                encoders.insert(parts[1].to_string());
            }
        }
    }
    let mut list: Vec<String> = encoders.into_iter().collect();
    list.sort();
    list
}

fn test_encoder(ffmpeg: &Path, encoder: &str) -> bool {
    let args = vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-nostdin".to_string(),
        "-f".to_string(),
        "lavfi".to_string(),
        "-i".to_string(),
        "color=c=black:s=256x256:d=0.04".to_string(),
        "-frames:v".to_string(),
        "1".to_string(),
        "-c:v".to_string(),
        encoder.to_string(),
        "-f".to_string(),
        "null".to_string(),
        "-".to_string(),
    ];
    portable::run_capture(ffmpeg, &args, Duration::from_secs(25))
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// GPU üreticisine göre donanım kodlayıcıları gerçek 1 karelik testle doğrular.
pub fn detect_hardware_encoders(
    ffmpeg: &Path,
    encoders: &[String],
    vendor: &str,
) -> EncoderSupport {
    let has = |name: &str| encoders.iter().any(|e| e == name);
    let test_group = |prefix: &str| CodecSupport {
        h264: has(&format!("h264_{prefix}")) && test_encoder(ffmpeg, &format!("h264_{prefix}")),
        hevc: has(&format!("hevc_{prefix}")) && test_encoder(ffmpeg, &format!("hevc_{prefix}")),
        av1: has(&format!("av1_{prefix}")) && test_encoder(ffmpeg, &format!("av1_{prefix}")),
    };

    let nvenc = if vendor == "nvidia" || has("h264_nvenc") {
        test_group("nvenc")
    } else {
        CodecSupport::default()
    };
    let amf = if vendor == "amd" || has("h264_amf") {
        test_group("amf")
    } else {
        CodecSupport::default()
    };
    let qsv = if vendor == "intel" || has("h264_qsv") {
        test_group("qsv")
    } else {
        CodecSupport::default()
    };

    let recommended = if nvenc.any() {
        "nvenc"
    } else if amf.any() {
        "amf"
    } else if qsv.any() {
        "qsv"
    } else {
        "cpu"
    };

    let mut note = String::new();
    if recommended == "cpu" {
        note = "No hardware encoder found; using CPU (libx264/libx265).".into();
    }

    EncoderSupport {
        vendor: vendor.into(),
        nvenc,
        amf,
        qsv,
        recommended: recommended.into(),
        note,
    }
}

// ---------------------------------------------------------------------------
//  Kurulum / güncelleme
// ---------------------------------------------------------------------------
pub async fn install_tool(app: &AppHandle, id: &str) -> Result<ToolInfo> {
    match id {
        "ffmpeg" => install_ffmpeg(app).await,
        "seven_zip" => install_sevenzip(app).await,
        "realesrgan" => {
            install_upscaler(
                app,
                "realesrgan",
                REALESRGAN_URL,
                "realesrgan-ncnn-vulkan",
                "realesrgan-ncnn-vulkan",
                "Real-ESRGAN",
                None,
            )
            .await
        }
        "waifu2x" => {
            install_upscaler(
                app,
                "waifu2x",
                WAIFU2X_URL,
                "waifu2x-ncnn-vulkan",
                "waifu2x-ncnn-vulkan",
                "Waifu2x",
                Some(WAIFU2X_SHA256),
            )
            .await
        }
        "qpdf" => {
            install_upscaler(
                app,
                "qpdf",
                QPDF_URL,
                "qpdf",
                "qpdf",
                "qpdf",
                Some(QPDF_SHA256),
            )
            .await
        }
        "ghostscript" => install_ghostscript(app).await,
        other => bail!("unknown tool: {other}"),
    }
}

fn emit_progress(
    app: &AppHandle,
    id: &str,
    phase: &str,
    downloaded: u64,
    total: u64,
    message: &str,
) {
    let percent = if total > 0 {
        (downloaded as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let _ = app.emit(
        "tool-progress",
        ToolProgress {
            id: id.into(),
            phase: phase.into(),
            downloaded,
            total,
            percent,
            message: message.into(),
        },
    );
}

async fn download_file(app: &AppHandle, id: &str, url: &str, target: &Path) -> Result<()> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

    let client = reqwest::Client::builder()
        .user_agent("ShrinkWise/3.0")
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(60 * 30))
        .build()?;
    emit_progress(app, id, "connect", 0, 0, "Connecting to server…");
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        bail!("download failed: HTTP {}", response.status());
    }
    let total = response.content_length().unwrap_or(0);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = tokio::fs::File::create(target).await?;
    let mut stream = response.bytes_stream();
    let mut downloaded = 0u64;
    let mut last_emit = std::time::Instant::now();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        if last_emit.elapsed() >= Duration::from_millis(120) {
            emit_progress(
                app,
                id,
                "download",
                downloaded,
                total,
                &format!("Downloading: {:.1} MB", downloaded as f64 / 1_048_576.0),
            );
            last_emit = std::time::Instant::now();
        }
    }
    file.flush().await?;
    emit_progress(app, id, "download", downloaded, total, "Download complete");
    Ok(())
}

async fn install_ffmpeg(app: &AppHandle) -> Result<ToolInfo> {
    #[cfg(not(windows))]
    {
        let _ = app;
        bail!("automatic FFmpeg installation is currently only available on Windows");
    }

    #[cfg(windows)]
    {
        let temp = portable::temp_dir().join(format!("ffmpeg-{}.zip", std::process::id()));
        let _ = fs::remove_file(&temp);
        download_file(app, "ffmpeg", FFMPEG_WIN_URL, &temp).await?;

        // Üretici sha256 dosyasını sunuyorsa doğrula.
        emit_progress(app, "ffmpeg", "verify", 0, 0, "Verifying file integrity…");
        let checksum =
            fetch_text("https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip.sha256")
                .await
                .ok();
        if let Some(expected) = checksum {
            let expected = expected
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim()
                .to_lowercase();
            if expected.len() == 64 {
                let actual = sha256_file(&temp)?;
                if actual != expected {
                    let _ = fs::remove_file(&temp);
                    bail!("downloaded FFmpeg checksum does not match");
                }
            }
        }

        emit_progress(app, "ffmpeg", "extract", 0, 0, "Extracting archive…");
        let target_dir = portable::tools_dir();
        fs::create_dir_all(&target_dir)?;
        let zip_path = temp.clone();
        let target_for_block = target_dir.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            extract_ffmpeg(&zip_path, &target_for_block)
        })
        .await
        .map_err(|e| anyhow!("extract task failed: {e}"))?;
        let _ = fs::remove_file(&temp);
        result?;

        emit_progress(app, "ffmpeg", "finish", 100, 100, "Verifying installation…");
        let ffmpeg_path = target_dir.join(exe_name("ffmpeg"));
        let ffprobe_path = target_dir.join(exe_name("ffprobe"));
        let version = version_of(
            &ffmpeg_path,
            &["-hide_banner", "-version"],
            "ffmpeg version",
        );
        emit_progress(app, "ffmpeg", "done", 100, 100, "FFmpeg ready");

        Ok(ToolInfo {
            id: "ffmpeg".into(),
            name: "FFmpeg".into(),
            path: Some(ffmpeg_path.display().to_string()),
            version,
            source: "portable".into(),
            available: true,
            required: true,
            note: format!("FFprobe: {}", ffprobe_path.display()),
        })
    }
}

async fn install_sevenzip(app: &AppHandle) -> Result<ToolInfo> {
    let target_dir = portable::tools_dir();
    fs::create_dir_all(&target_dir)?;

    #[cfg(windows)]
    for sys_dir in [r"C:\Program Files\7-Zip", r"C:\Program Files (x86)\7-Zip"] {
        let sys_exe = Path::new(sys_dir).join("7z.exe");
        let sys_dll = Path::new(sys_dir).join("7z.dll");
        if sys_exe.is_file() && sys_dll.is_file() {
            let dest_exe = target_dir.join("7z.exe");
            let dest_dll = target_dir.join("7z.dll");
            let _ = fs::copy(&sys_exe, &dest_exe);
            let _ = fs::copy(&sys_dll, &dest_dll);
            let _ = fs::remove_file(target_dir.join("7zr.exe"));
            if dest_exe.is_file() {
                let ver = seven_zip_version(&dest_exe);
                emit_progress(app, "seven_zip", "done", 100, 100, "7-Zip ready");
                return Ok(ToolInfo {
                    id: "seven_zip".into(),
                    name: "7-Zip".into(),
                    path: Some(dest_exe.display().to_string()),
                    version: ver,
                    source: "portable".into(),
                    available: true,
                    required: false,
                    note: "For tighter archive packing".into(),
                });
            }
        }
    }

    let target = target_dir.join(exe_name("7zr"));
    let temp = target.with_extension("download");
    download_file(app, "seven_zip", SEVENZIP_URL, &temp).await?;
    if target.exists() {
        let _ = fs::remove_file(&target);
    }
    fs::rename(&temp, &target).context("could not move 7zr.exe")?;
    let ver = seven_zip_version(&target);
    emit_progress(app, "seven_zip", "done", 100, 100, "7-Zip ready");
    Ok(ToolInfo {
        id: "seven_zip".into(),
        name: "7-Zip".into(),
        path: Some(target.display().to_string()),
        version: ver,
        source: "portable".into(),
        available: true,
        required: false,
        note: "For tighter archive packing".into(),
    })
}

async fn install_upscaler(
    app: &AppHandle,
    id: &str,
    url: &str,
    folder: &str,
    exe_base: &str,
    name: &str,
    checksum: Option<&str>,
) -> Result<ToolInfo> {
    #[cfg(not(windows))]
    {
        let _ = (app, id, url, folder, exe_base, name, checksum);
        bail!("automatic {name} installation is currently only available on Windows");
    }

    #[cfg(windows)]
    {
        let temp = portable::temp_dir().join(format!("{folder}-{}.zip", std::process::id()));
        let _ = fs::remove_file(&temp);
        download_file(app, id, url, &temp).await?;

        if let Some(expected) = checksum {
            emit_progress(app, id, "verify", 0, 0, "Verifying archive…");
            let actual = sha256_file(&temp)?;
            if !actual.eq_ignore_ascii_case(expected) {
                let _ = fs::remove_file(&temp);
                bail!("{name} archive failed checksum verification; download aborted");
            }
        }

        emit_progress(app, id, "extract", 0, 0, "Extracting archive…");
        let target_dir = portable::tools_dir().join(folder);
        let zip_path = temp.clone();
        let exe_base_owned = exe_base.to_string();
        let extracted = tauri::async_runtime::spawn_blocking(move || {
            extract_tool_zip(&zip_path, &target_dir, &exe_base_owned)
        })
        .await
        .map_err(|e| anyhow!("extract task failed: {e}"))?;
        let _ = fs::remove_file(&temp);
        let exe = extracted?;

        emit_progress(app, id, "done", 100, 100, &format!("{name} ready"));
        let ver = match id {
            "realesrgan" => Some("v0.2.5.0 (ncnn-vulkan)".to_string()),
            "waifu2x" => Some("20250915 (ncnn-vulkan)".to_string()),
            "qpdf" => version_of(&exe, &["--version"], "qpdf version"),
            _ => None,
        };
        Ok(ToolInfo {
            id: id.into(),
            name: name.into(),
            path: Some(exe.display().to_string()),
            version: ver,
            source: "portable".into(),
            available: true,
            required: false,
            note: String::new(),
        })
    }
}

async fn install_ghostscript(app: &AppHandle) -> Result<ToolInfo> {
    #[cfg(not(windows))]
    {
        let _ = app;
        bail!("automatic Ghostscript installation is currently only available on Windows");
    }

    #[cfg(windows)]
    {
        let temp = portable::temp_dir().join(format!("gs10080w64-{}.exe", std::process::id()));
        let _ = fs::remove_file(&temp);
        download_file(app, "ghostscript", GHOSTSCRIPT_URL, &temp).await?;

        emit_progress(app, "ghostscript", "verify", 0, 0, "Verifying archive…");
        let actual = sha256_file(&temp)?;
        if !actual.eq_ignore_ascii_case(GHOSTSCRIPT_SHA256) {
            let _ = fs::remove_file(&temp);
            bail!("Ghostscript installer failed checksum verification; download aborted");
        }

        emit_progress(app, "ghostscript", "extract", 0, 0, "Extracting archive…");
        let target_dir = portable::tools_dir().join("ghostscript");
        let temp_for_block = temp.clone();
        let target_for_block = target_dir.clone();
        let exe = tauri::async_runtime::spawn_blocking(move || {
            extract_ghostscript(&temp_for_block, &target_for_block)
        })
        .await
        .map_err(|e| anyhow!("extract task failed: {e}"))??;
        let _ = fs::remove_file(&temp);

        let ver = version_of(&exe, &["--version"], "");
        emit_progress(app, "ghostscript", "done", 100, 100, "Ghostscript ready");
        Ok(ToolInfo {
            id: "ghostscript".into(),
            name: "Ghostscript".into(),
            path: Some(exe.display().to_string()),
            version: ver,
            source: "portable".into(),
            available: true,
            required: false,
            note: String::new(),
        })
    }
}

#[cfg(windows)]
fn extract_ghostscript(installer_path: &Path, target_dir: &Path) -> Result<PathBuf> {
    if target_dir.exists() {
        let _ = fs::remove_dir_all(target_dir);
    }
    fs::create_dir_all(target_dir)?;

    if let Some((seven_zip, _)) = find_tool(&["7z"], "", &[]) {
        let out_arg = format!("-o{}", target_dir.display());
        let args = vec![
            "x".to_string(),
            installer_path.display().to_string(),
            out_arg,
            "-y".to_string(),
        ];
        let _ = portable::run_capture(&seven_zip, &args, Duration::from_secs(120));
    }

    let candidate_bin = target_dir.join("bin").join("gswin64c.exe");
    if candidate_bin.is_file() {
        return Ok(candidate_bin);
    }

    let args = vec!["/S".to_string(), format!("/D={}", target_dir.display())];
    let _ = portable::run_capture(installer_path, &args, Duration::from_secs(180));
    if candidate_bin.is_file() {
        return Ok(candidate_bin);
    }
    let candidate_root = target_dir.join("gswin64c.exe");
    if candidate_root.is_file() {
        return Ok(candidate_root);
    }
    bail!(
        "gswin64c.exe not found after extracting Ghostscript; install 7-Zip first for portable extraction"
    )
}

async fn fetch_text(url: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .user_agent("ShrinkWise/3.0")
        .timeout(Duration::from_secs(20))
        .build()?;
    let text = client.get(url).send().await?.text().await?;
    Ok(text)
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 512];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn extract_ffmpeg(zip_path: &Path, target_dir: &Path) -> Result<()> {
    let file = File::open(zip_path).context("could not open ffmpeg archive")?;
    let mut archive = zip::ZipArchive::new(file).context("invalid ffmpeg archive")?;
    let mut found_ffmpeg = false;
    let mut found_ffprobe = false;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if !entry.is_file() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        let lower = name.to_lowercase();
        let is_ffmpeg = lower.ends_with("/ffmpeg.exe") || lower == "ffmpeg.exe";
        let is_ffprobe = lower.ends_with("/ffprobe.exe") || lower == "ffprobe.exe";
        if !is_ffmpeg && !is_ffprobe {
            continue;
        }
        let file_name = if is_ffmpeg {
            "ffmpeg.exe"
        } else {
            "ffprobe.exe"
        };
        let target = target_dir.join(file_name);
        let tmp = target.with_extension("new");
        let mut out = File::create(&tmp)?;
        io::copy(&mut entry, &mut out)?;
        out.flush()?;
        drop(out);
        if target.exists() {
            fs::remove_file(&target).with_context(|| {
                format!(
                    "could not delete old {} (FFmpeg may be running). Close the app and try again.",
                    file_name
                )
            })?;
        }
        fs::rename(&tmp, &target)?;
        if is_ffmpeg {
            found_ffmpeg = true;
        }
        if is_ffprobe {
            found_ffprobe = true;
        }
    }
    if !found_ffmpeg || !found_ffprobe {
        bail!("ffmpeg.exe / ffprobe.exe not found in archive");
    }
    Ok(())
}

fn extract_tool_zip(zip_path: &Path, target_dir: &Path, exe_base: &str) -> Result<PathBuf> {
    let file = File::open(zip_path)
        .with_context(|| format!("could not open archive: {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("invalid archive")?;

    let exe_file = exe_name(exe_base).to_lowercase();
    let mut prefix = None;
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        let name = entry.name().replace('\\', "/").to_lowercase();
        if name == exe_file || name.ends_with(&format!("/{exe_file}")) {
            prefix = Some(match name.rfind('/') {
                Some(pos) => name[..=pos].to_string(),
                None => String::new(),
            });
            break;
        }
    }
    let prefix = prefix.ok_or_else(|| anyhow!("{exe_file} not found in archive"))?;

    if target_dir.exists() {
        fs::remove_dir_all(target_dir)
            .with_context(|| format!("could not clear {}", target_dir.display()))?;
    }
    fs::create_dir_all(target_dir)?;

    let mut extracted = 0usize;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        let lower = name.to_lowercase();
        if !prefix.is_empty() && !lower.starts_with(&prefix) {
            continue;
        }
        let relative = &name[prefix.len().min(name.len())..];
        if relative.is_empty() {
            continue;
        }
        let relative = Path::new(relative);
        if relative
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            continue;
        }
        let target = target_dir.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = File::create(&target)?;
        io::copy(&mut entry, &mut out)?;
        out.flush()?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&target, fs::Permissions::from_mode(mode));
        }
        extracted += 1;
    }
    if extracted == 0 {
        bail!("archive did not contain any files");
    }

    let exe = target_dir.join(exe_name(exe_base));
    if !exe.is_file() {
        bail!("{exe_file} missing after extraction");
    }
    Ok(exe)
}

pub fn tool_path(settings: &Settings, id: &str, extra: &[PathBuf]) -> Option<PathBuf> {
    let status = probe(settings, extra, false);
    let info = match id {
        "ffmpeg" => status.ffmpeg,
        "ffprobe" => status.ffprobe,
        "seven_zip" => status.seven_zip,
        "realesrgan" => status.realesrgan,
        "waifu2x" => status.waifu2x,
        "ghostscript" => status.ghostscript,
        "qpdf" => status.qpdf,
        _ => return None,
    };
    info.path.map(PathBuf::from)
}
