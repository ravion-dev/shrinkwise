use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use filetime::FileTime;
use rayon::prelude::*;
use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::classifier::{self, Task};
use crate::container_engine;
use crate::ffmpeg_engine;
use crate::image_engine;
use crate::pdf_engine;
use crate::portable;
use crate::settings::Settings;
use crate::text_engine;
use crate::tools;

static TASK_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub rel: String,
    pub kind: String,
    pub status: String,
    pub in_bytes: u64,
    pub out_bytes: u64,
    pub saved_bytes: u64,
    pub ratio: f64,
    pub out_path: String,
    pub method: String,
    pub note: String,
    pub error: String,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KindStat {
    pub files: u64,
    pub in_bytes: u64,
    pub out_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct JobStats {
    pub total_files: u64,
    pub processed: u64,
    pub compressed: u64,
    pub copied: u64,
    pub failed: u64,
    pub in_bytes: u64,
    pub out_bytes: u64,
    pub saved_bytes: u64,
    pub ratio: f64,
    pub elapsed_ms: u64,
    pub cancelled: bool,
    pub by_kind: HashMap<String, KindStat>,
}

pub struct JobManager {
    pub running: AtomicBool,
    pub cancel: Arc<AtomicBool>,
    pub last_outcomes: Mutex<Vec<Outcome>>,
    pub last_stats: Mutex<Option<JobStats>>,
}

impl Default for JobManager {
    fn default() -> Self {
        Self {
            running: AtomicBool::new(false),
            cancel: Arc::new(AtomicBool::new(false)),
            last_outcomes: Mutex::new(Vec::new()),
            last_stats: Mutex::new(None),
        }
    }
}

struct RunningGuard<'a>(&'a AtomicBool);
impl Drop for RunningGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

struct JobContext {
    app: AppHandle,
    settings: Settings,
    ffmpeg: Option<PathBuf>,
    ffprobe: Option<PathBuf>,
    encoders: Vec<String>,
    ghostscript: Option<PathBuf>,
    qpdf: Option<PathBuf>,
    total_bytes: u64,
    total_files: usize,
    completed_bytes: AtomicU64,
    completed_files: AtomicUsize,
    in_progress: Mutex<HashMap<String, f64>>,
    cancel: Arc<AtomicBool>,
    stats: Mutex<JobStats>,
    /// Output paths already claimed by this job. Prevents two source files that
    /// map to the same target name (for example `clip.mov` + `clip.mp4` ->
    /// `clip.mp4`) from silently overwriting each other.
    reserved: Mutex<HashSet<PathBuf>>,
}

impl JobContext {
    fn emit(&self, value: serde_json::Value) {
        let _ = self.app.emit("job-event", value);
    }

    fn log(&self, message: impl Into<String>, level: &str) {
        self.emit(json!({ "event": "log", "message": message.into(), "level": level }));
    }

    fn emit_stats(&self) {
        if let Ok(stats) = self.stats.lock() {
            self.emit(json!({ "event": "stats", "stats": &*stats }));
        }
    }

    fn progress(&self, rel: &str, file_size: u64, file_fraction: f32) {
        let fraction = file_fraction.clamp(0.0, 1.0) as f64;
        let contribution = file_size as f64 * fraction;
        let in_progress_sum = {
            let mut map = self.in_progress.lock().unwrap_or_else(|e| e.into_inner());
            map.insert(rel.to_string(), contribution);
            map.values().sum::<f64>()
        };
        let done = self.completed_bytes.load(Ordering::Relaxed) as f64;
        let overall = ((done + in_progress_sum) / self.total_bytes.max(1) as f64).clamp(0.0, 1.0);
        self.emit(json!({
            "event": "progress",
            "overall": overall,
            "fileFraction": file_fraction,
            "file": rel,
            "done": self.completed_files.load(Ordering::Relaxed),
            "total": self.total_files,
        }));
    }
}

fn fmt_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn sync_mtime(src: &Path, dst: &Path, keep_mtime: bool) {
    if keep_mtime
        && let Ok(meta) = fs::metadata(src)
        && let Ok(modified) = meta.modified()
    {
        let _ = filetime::set_file_mtime(dst, FileTime::from_system_time(modified));
    }
}

fn copy_original(src: &Path, dst: &Path, keep_mtime: bool) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    // Copy next to the destination first so an interrupted copy never destroys
    // an existing destination file.
    let parent = dst.parent().unwrap_or_else(|| Path::new("."));
    let temp = parent.join(format!(
        ".swcopy-{}-{}",
        std::process::id(),
        TASK_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    if let Err(err) =
        fs::copy(src, &temp).with_context(|| format!("could not copy: {}", src.display()))
    {
        let _ = fs::remove_file(&temp);
        return Err(err);
    }
    replace_file(&temp, dst)?;
    sync_mtime(src, dst, keep_mtime);
    Ok(())
}

fn copy_fallback_replace(
    temp: &Path,
    dst: &Path,
    backup: &Path,
    had_old: bool,
    rename_err: std::io::Error,
) -> Result<()> {
    // Different volume: fall back to copy, but keep the old file until
    // the new bytes are safely in place.
    match fs::copy(temp, dst) {
        Ok(_) => {
            let _ = fs::remove_file(temp);
            if had_old {
                let _ = fs::remove_file(backup);
            }
            Ok(())
        }
        Err(copy_err) => {
            let _ = fs::remove_file(dst);
            if had_old {
                let _ = fs::rename(backup, dst);
            }
            let _ = fs::remove_file(temp);
            Err(copy_err).with_context(|| {
                format!(
                    "could not write destination (rename failed: {rename_err}): {}",
                    dst.display()
                )
            })
        }
    }
}

fn replace_file(temp: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    let parent = dst.parent().unwrap_or_else(|| Path::new("."));
    let had_old = dst.exists();
    let backup = parent.join(format!(
        ".swbak-{}-{}",
        std::process::id(),
        TASK_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    if had_old {
        fs::rename(dst, &backup).with_context(|| {
            format!(
                "could not move existing destination aside: {}",
                dst.display()
            )
        })?;
    }
    match fs::rename(temp, dst) {
        Ok(()) => {
            if had_old {
                let _ = fs::remove_file(&backup);
            }
            Ok(())
        }
        Err(rename_err) => copy_fallback_replace(temp, dst, &backup, had_old, rename_err),
    }
}

fn final_path(dst_root: &Path, task: &Task, ext: &str, add_extension: bool) -> PathBuf {
    let rel = &task.rel;
    let parent = rel.parent().unwrap_or_else(|| Path::new(""));
    let name = rel.file_name().and_then(|s| s.to_str()).unwrap_or("output");
    let new_name = if add_extension {
        if ext.is_empty() || name.to_lowercase().ends_with(&format!(".{ext}")) {
            name.to_string()
        } else {
            format!("{name}.{ext}")
        }
    } else if ext.is_empty() {
        name.to_string()
    } else {
        let stem = Path::new(name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(name);
        format!("{stem}.{ext}")
    };
    dst_root.join(parent).join(new_name)
}

fn disambiguate(path: &Path, n: u32) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    match path.extension().and_then(|s| s.to_str()) {
        Some(ext) if !ext.is_empty() => {
            let suffix = format!(".{ext}");
            let stem = name.strip_suffix(&suffix).unwrap_or(name);
            parent.join(format!("{stem} ({n}).{ext}"))
        }
        _ => parent.join(format!("{name} ({n})")),
    }
}

/// Reserves the final output path for this job. Different kinds can map to the
/// same name (`clip.mov` and `clip.mp4` both become `clip.mp4`), so the second
/// one gets a ` (2)` suffix instead of silently overwriting the first.
fn claim_path(
    ctx: &JobContext,
    dst_root: &Path,
    task: &Task,
    ext: &str,
    add_extension: bool,
) -> PathBuf {
    let base = final_path(dst_root, task, ext, add_extension);
    let mut reserved = ctx.reserved.lock().unwrap_or_else(|e| e.into_inner());
    if reserved.insert(base.clone()) {
        return base;
    }
    let mut n = 2u32;
    loop {
        let candidate = disambiguate(&base, n);
        if reserved.insert(candidate.clone()) {
            return candidate;
        }
        n = n.saturating_add(1);
    }
}

fn temp_output(dst_dir: &Path, ext: &str, tag: &str) -> PathBuf {
    dst_dir.join(format!(
        ".sw-{}-{}-{}.{}",
        tag,
        std::process::id(),
        TASK_COUNTER.fetch_add(1, Ordering::Relaxed),
        if ext.is_empty() { "tmp" } else { ext }
    ))
}

fn make_outcome(
    task: &Task,
    status: &str,
    out_bytes: u64,
    out_path: &Path,
    details: (&str, &str, &str),
    started: Instant,
) -> Outcome {
    let (method, note, error) = details;
    let saved = task.size.saturating_sub(out_bytes);
    Outcome {
        rel: task.rel.to_string_lossy().replace('\\', "/"),
        kind: task.kind.clone(),
        status: status.into(),
        in_bytes: task.size,
        out_bytes,
        saved_bytes: if status == "compressed" { saved } else { 0 },
        ratio: if task.size > 0 {
            1.0 - out_bytes as f64 / task.size as f64
        } else {
            0.0
        },
        out_path: out_path.display().to_string(),
        method: method.into(),
        note: note.into(),
        error: error.into(),
        elapsed_ms: started.elapsed().as_millis() as u64,
    }
}

fn copy_fallback(
    ctx: &JobContext,
    task: &Task,
    dst_root: &Path,
    note: &str,
    started: Instant,
) -> Outcome {
    let dst = claim_path(ctx, dst_root, task, "", false);
    match copy_original(&task.src, &dst, ctx.settings.keep_mtime) {
        Ok(()) => make_outcome(task, "copied", task.size, &dst, ("copy", note, ""), started),
        Err(err) => make_outcome(
            task,
            "failed",
            task.size,
            &dst,
            ("", "", &err.to_string()),
            started,
        ),
    }
}

fn finalize_compressed(
    ctx: &JobContext,
    task: &Task,
    dst_root: &Path,
    temp: &Path,
    target: (&str, bool),
    note: &str,
    started: Instant,
) -> Outcome {
    let (ext, add_extension) = target;
    let out_size = fs::metadata(temp).map(|m| m.len()).unwrap_or(0);
    if ctx.settings.keep_larger && out_size >= task.size {
        let _ = fs::remove_file(temp);
        return copy_fallback(ctx, task, dst_root, "no gain", started);
    }
    let dst = claim_path(ctx, dst_root, task, ext, add_extension);
    match replace_file(temp, &dst) {
        Ok(()) => {
            sync_mtime(&task.src, &dst, ctx.settings.keep_mtime);
            make_outcome(
                task,
                "compressed",
                out_size,
                &dst,
                (note, note, ""),
                started,
            )
        }
        Err(err) => make_outcome(
            task,
            "failed",
            task.size,
            &dst,
            ("", "", &err.to_string()),
            started,
        ),
    }
}

fn process_text_task(
    ctx: &JobContext,
    task: &Task,
    dst_root: &Path,
    dst_dir: &Path,
    started: Instant,
) -> Outcome {
    match text_engine::compress(&task.src, dst_dir, ctx.settings.lossy, 128 * 1024 * 1024) {
        Ok((temp, ext, note)) => {
            if ctx.cancel.load(Ordering::Relaxed) {
                let _ = fs::remove_file(&temp);
                return make_outcome(task, "cancelled", 0, Path::new(""), ("", "", ""), started);
            }
            finalize_compressed(ctx, task, dst_root, &temp, (&ext, true), &note, started)
        }
        Err(err) => copy_fallback(ctx, task, dst_root, &err.to_string(), started),
    }
}

fn process_ffmpeg_task(
    ctx: &JobContext,
    task: &Task,
    temp: &Path,
    progress: &(dyn Fn(f32) + Sync),
) -> Result<Option<String>> {
    let (Some(ffmpeg), Some(ffprobe)) = (ctx.ffmpeg.as_deref(), ctx.ffprobe.as_deref()) else {
        return Ok(None);
    };
    let env = ffmpeg_engine::FfmpegEnv {
        ffmpeg,
        ffprobe,
        encoders: &ctx.encoders,
        cancel: &ctx.cancel,
    };
    let note = match task.kind.as_str() {
        "video" => ffmpeg_engine::compress_video(&task.src, temp, &ctx.settings, &env, progress)?,
        "audio" => ffmpeg_engine::compress_audio(&task.src, temp, &ctx.settings, &env, progress)?,
        _ => ffmpeg_engine::compress_gif(&task.src, temp, &ctx.settings, &env, progress)?,
    };
    if ctx.settings.verify && !ffmpeg_engine::verify_output(ffprobe, temp) {
        bail!("output verification failed ({})", task.kind);
    }
    Ok(Some(note))
}

fn run_media_engine(
    ctx: &JobContext,
    task: &Task,
    temp: &Path,
    progress: &(dyn Fn(f32) + Sync),
) -> Result<Option<String>> {
    match task.kind.as_str() {
        "image" => {
            let note =
                image_engine::compress(&task.src, temp, &ctx.settings, ctx.ffmpeg.as_deref())?;
            if ctx.settings.verify && !image_engine::verify(temp, ctx.ffprobe.as_deref()) {
                bail!("output verification failed (image)");
            }
            Ok(Some(note))
        }
        "video" | "audio" | "gif" => process_ffmpeg_task(ctx, task, temp, progress),
        "svg" => text_engine::minify_svg(&task.src, temp).map(Some),
        "pdf" => pdf_engine::compress(
            &task.src,
            temp,
            &ctx.settings,
            ctx.ghostscript.as_deref(),
            ctx.qpdf.as_deref(),
        )
        .map(Some),
        "office" | "archive" => {
            let depth = if ctx.settings.zip_deep && ctx.settings.archive_mode != "outer" {
                ctx.settings.zip_deep_depth.max(1)
            } else {
                0
            };
            container_engine::compress_zip(&task.src, temp, depth).map(Some)
        }
        _ => Ok(None),
    }
}

fn process_media_task(
    ctx: &JobContext,
    task: &Task,
    dst_root: &Path,
    dst_dir: &Path,
    started: Instant,
) -> Outcome {
    if !matches!(
        task.kind.as_str(),
        "image" | "video" | "audio" | "gif" | "svg" | "pdf" | "office" | "archive"
    ) {
        return copy_fallback(ctx, task, dst_root, "", started);
    }
    let ext = task.preferred_ext.clone();
    let temp = temp_output(dst_dir, &ext, &task.kind);
    let rel = task.rel.to_string_lossy().replace('\\', "/");
    let media_progress = |fraction: f32| {
        ctx.progress(&rel, task.size, fraction);
    };

    let result = run_media_engine(ctx, task, &temp, &media_progress);
    if ctx.cancel.load(Ordering::Relaxed) {
        let _ = fs::remove_file(&temp);
        return make_outcome(task, "cancelled", 0, Path::new(""), ("", "", ""), started);
    }
    match result {
        Ok(Some(note)) => {
            finalize_compressed(ctx, task, dst_root, &temp, (&ext, false), &note, started)
        }
        Ok(None) => {
            let _ = fs::remove_file(&temp);
            copy_fallback(ctx, task, dst_root, "FFmpeg missing", started)
        }
        Err(err) => {
            let _ = fs::remove_file(&temp);
            copy_fallback(ctx, task, dst_root, &err.to_string(), started)
        }
    }
}

fn process_task(ctx: &JobContext, task: &Task, dst_root: &Path) -> Outcome {
    let started = Instant::now();
    if ctx.cancel.load(Ordering::Relaxed) {
        return make_outcome(task, "cancelled", 0, Path::new(""), ("", "", ""), started);
    }
    let dst_dir = dst_root.join(task.rel.parent().unwrap_or_else(|| Path::new("")));
    if let Err(err) = fs::create_dir_all(&dst_dir) {
        return make_outcome(
            task,
            "failed",
            task.size,
            &dst_dir,
            ("", "", &format!("could not create folder: {err}")),
            started,
        );
    }
    if ctx.settings.dry_run {
        return make_outcome(
            task,
            "skipped",
            task.size,
            &dst_dir,
            ("", "dry run", ""),
            started,
        );
    }
    if task.kind == "copy" {
        let note = task
            .src
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{e}"))
            .unwrap_or_default();
        return copy_fallback(ctx, task, dst_root, &note, started);
    }
    if task.kind == "text" || task.kind == "lossless" {
        process_text_task(ctx, task, dst_root, &dst_dir, started)
    } else {
        process_media_task(ctx, task, dst_root, &dst_dir, started)
    }
}

fn record(stats: &mut JobStats, outcome: &Outcome) {
    stats.processed += 1;
    stats.in_bytes += outcome.in_bytes;
    stats.out_bytes += outcome.out_bytes;
    match outcome.status.as_str() {
        "compressed" => stats.compressed += 1,
        "copied" => stats.copied += 1,
        "failed" => stats.failed += 1,
        _ => {}
    }
    let entry = stats.by_kind.entry(outcome.kind.clone()).or_default();
    entry.files += 1;
    entry.in_bytes += outcome.in_bytes;
    entry.out_bytes += outcome.out_bytes;
    stats.saved_bytes = stats.in_bytes.saturating_sub(stats.out_bytes);
    stats.ratio = if stats.in_bytes > 0 {
        1.0 - stats.out_bytes as f64 / stats.in_bytes as f64
    } else {
        0.0
    };
}

fn prepare_tasks(
    app: &AppHandle,
    src: &Path,
    dst: &Path,
    settings: &Settings,
    ffmpeg_ok: bool,
    only: Option<Vec<String>>,
) -> Option<Vec<Task>> {
    let _ = app.emit(
        "job-event",
        json!({ "event": "log", "message": format!("Scanning: {}", src.display()), "level": "dim" }),
    );
    let mut tasks = match classifier::collect_tasks(src, dst, settings, ffmpeg_ok) {
        Ok(tasks) => tasks,
        Err(err) => {
            let _ = app.emit(
                "job-event",
                json!({ "event": "log", "message": format!("Scan error: {err}"), "level": "error" }),
            );
            let _ = app.emit(
                "job-event",
                json!({ "event": "done", "stats": JobStats::default() }),
            );
            return None;
        }
    };
    // Retry runs only contain the relative paths the previous run failed on.
    if let Some(only) = only {
        let wanted: HashSet<String> = only.into_iter().collect();
        if !wanted.is_empty() {
            tasks.retain(|task| wanted.contains(&task.rel.to_string_lossy().replace('\\', "/")));
            let _ = app.emit(
                "job-event",
                json!({ "event": "log", "message": format!("Retrying {} selected files.", tasks.len()), "level": "accent" }),
            );
        }
    }
    Some(tasks)
}

fn abort_job(
    app: &AppHandle,
    manager: &JobManager,
    stats: &JobStats,
    message: String,
    level: &str,
) {
    let _ = app.emit(
        "job-event",
        json!({ "event": "log", "message": message, "level": level }),
    );
    *manager.last_stats.lock().unwrap_or_else(|e| e.into_inner()) = Some(stats.clone());
    manager
        .last_outcomes
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
    let _ = app.emit("job-event", json!({ "event": "done", "stats": stats }));
}

fn check_preflight(
    app: &AppHandle,
    dst: &Path,
    total_bytes: u64,
    settings: &Settings,
    stats: &JobStats,
    manager: &JobManager,
) -> bool {
    if stats.total_files == 0 {
        abort_job(app, manager, stats, "No files to process.".into(), "warn");
        return false;
    }
    let _ = app.emit("job-event", json!({
        "event": "log",
        "message": format!("{} files • {} • {} workers • lossy {}%", stats.total_files, fmt_size(total_bytes), settings.workers, settings.lossy),
        "level": "accent"
    }));
    if let Some(free) = portable::available_space(dst) {
        if free < 64 * 1024 * 1024 {
            let msg = format!(
                "Not enough free space in the destination ({} available). Free some space and try again.",
                fmt_size(free)
            );
            abort_job(app, manager, stats, msg, "error");
            return false;
        }
        if free < total_bytes {
            let _ = app.emit("job-event", json!({
                "event": "log",
                "message": format!("Low disk space: {} available, source total is {}. Files may fail if the disk fills up.", fmt_size(free), fmt_size(total_bytes)),
                "level": "warn"
            }));
        }
    }
    true
}

fn emit_outcome_log(ctx: &JobContext, result: &Outcome) {
    let pct = if result.in_bytes > 0 {
        result.ratio * 100.0
    } else {
        0.0
    };
    match result.status.as_str() {
        "compressed" => ctx.emit(json!({
            "event": "log",
            "message": format!("{}: {} -> {} ({:.0}% smaller)  [{}]", result.rel, fmt_size(result.in_bytes), fmt_size(result.out_bytes), pct, result.method),
            "level": if pct >= 1.0 { "ok" } else { "dim" }
        })),
        "copied" => ctx.emit(json!({
            "event": "log",
            "message": format!("{}: copied{}", result.rel, if result.note.is_empty() { String::new() } else { format!(" ({})", result.note) }),
            "level": "dim"
        })),
        "failed" => ctx.emit(json!({
            "event": "log",
            "message": format!("{}: ERROR - {}", result.rel, result.error),
            "level": "error"
        })),
        _ => {}
    }
}

fn emit_task_completion(
    ctx: &JobContext,
    task_size: u64,
    result: Outcome,
    outcomes: &Mutex<Vec<Outcome>>,
) {
    {
        let mut stats = ctx.stats.lock().unwrap_or_else(|e| e.into_inner());
        record(&mut stats, &result);
    }
    ctx.in_progress
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&result.rel);
    ctx.completed_bytes
        .fetch_add(task_size.max(1), Ordering::Relaxed);
    ctx.completed_files.fetch_add(1, Ordering::Relaxed);

    if let Ok(stats) = ctx.stats.lock() {
        ctx.emit(json!({ "event": "stats", "stats": &*stats }));
    }
    ctx.emit(json!({
        "event": "file", "rel": result.rel, "kind": result.kind, "status": result.status,
        "inBytes": result.in_bytes, "outBytes": result.out_bytes, "ratio": result.ratio,
        "method": result.method, "note": result.note, "error": result.error,
    }));
    emit_outcome_log(ctx, &result);
    outcomes
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(result);
    let done = ctx.completed_files.load(Ordering::Relaxed);
    let overall =
        ctx.completed_bytes.load(Ordering::Relaxed) as f64 / ctx.total_bytes.max(1) as f64;
    ctx.emit(json!({
        "event": "progress", "overall": overall, "fileFraction": 1.0, "file": "",
        "done": done, "total": ctx.total_files,
    }));
}

fn process_single_task(ctx: &JobContext, task: &Task, dst: &Path, outcomes: &Mutex<Vec<Outcome>>) {
    if ctx.cancel.load(Ordering::Relaxed) {
        return;
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        process_task(ctx, task, dst)
    }))
    .unwrap_or_else(|_| {
        let dst_dir = dst.join(task.rel.parent().unwrap_or_else(|| Path::new("")));
        let _ = fs::create_dir_all(&dst_dir);
        make_outcome(
            task,
            "failed",
            task.size,
            &dst_dir,
            ("", "", "unexpected internal error"),
            Instant::now(),
        )
    });
    if result.status == "cancelled" {
        return;
    }
    emit_task_completion(ctx, task.size, result, outcomes);
}

fn finish_job(
    ctx: &JobContext,
    manager: &JobManager,
    src: &Path,
    dst: &Path,
    outcomes: &Mutex<Vec<Outcome>>,
    started: Instant,
) {
    let mut final_stats = ctx.stats.lock().unwrap_or_else(|e| e.into_inner()).clone();
    final_stats.elapsed_ms = started.elapsed().as_millis() as u64;
    final_stats.cancelled = ctx.cancel.load(Ordering::Relaxed);
    final_stats.saved_bytes = final_stats.in_bytes.saturating_sub(final_stats.out_bytes);
    final_stats.ratio = if final_stats.in_bytes > 0 {
        1.0 - final_stats.out_bytes as f64 / final_stats.in_bytes as f64
    } else {
        0.0
    };

    let all_outcomes = outcomes.lock().unwrap_or_else(|e| e.into_inner()).clone();
    *manager
        .last_outcomes
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = all_outcomes;
    *manager.last_stats.lock().unwrap_or_else(|e| e.into_inner()) = Some(final_stats.clone());

    let finished_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0);
    let history_entry = json!({
        "finishedAt": finished_at, "src": src.display().to_string(),
        "dst": dst.display().to_string(), "stats": &final_stats,
    });
    portable::append_history(&history_entry.to_string());
    ctx.emit(json!({
        "event": "log",
        "message": format!(
            "{} • {} -> {} • {:.1}% saved • {:.1} s{}",
            if final_stats.cancelled { "Cancelled" } else { "Completed" },
            fmt_size(final_stats.in_bytes), fmt_size(final_stats.out_bytes),
            final_stats.ratio * 100.0, final_stats.elapsed_ms as f64 / 1000.0,
            if final_stats.failed > 0 { format!(" • {} failed", final_stats.failed) } else { String::new() }
        ),
        "level": if final_stats.failed > 0 { "warn" } else { "ok" }
    }));
    ctx.emit(json!({ "event": "done", "stats": final_stats }));
}

fn build_scan_stats(tasks: &[Task]) -> JobStats {
    let mut stats = JobStats {
        total_files: tasks.len() as u64,
        ..Default::default()
    };
    for task in tasks {
        stats.by_kind.entry(task.kind.clone()).or_default();
    }
    stats
}

fn build_job_context(
    app: &AppHandle,
    settings: &Settings,
    tools_status: &tools::ToolsStatus,
    total_files: usize,
    total_bytes: u64,
    manager: &JobManager,
    stats: JobStats,
) -> JobContext {
    JobContext {
        app: app.clone(),
        settings: settings.clone(),
        ffmpeg: tools_status.ffmpeg.path.as_ref().map(PathBuf::from),
        ffprobe: tools_status.ffprobe.path.as_ref().map(PathBuf::from),
        encoders: tools_status.encoders.clone(),
        ghostscript: tools_status.ghostscript.path.as_ref().map(PathBuf::from),
        qpdf: tools_status.qpdf.path.as_ref().map(PathBuf::from),
        total_bytes,
        total_files,
        completed_bytes: AtomicU64::new(0),
        completed_files: AtomicUsize::new(0),
        in_progress: Mutex::new(HashMap::new()),
        cancel: manager.cancel.clone(),
        stats: Mutex::new(stats),
        reserved: Mutex::new(HashSet::new()),
    }
}

pub fn run_job(
    app: &AppHandle,
    src: PathBuf,
    dst: PathBuf,
    settings: Settings,
    extra_dirs: Vec<PathBuf>,
    manager: &JobManager,
    only: Option<Vec<String>>,
) {
    let _guard = RunningGuard(&manager.running);
    manager.cancel.store(false, Ordering::SeqCst);
    let started = Instant::now();

    let tools_status = tools::probe(&settings, &extra_dirs, true);
    let ffmpeg_ok = tools_status.ffmpeg.path.is_some();
    let Some(tasks) = prepare_tasks(app, &src, &dst, &settings, ffmpeg_ok, only) else {
        return;
    };

    let total_bytes: u64 = tasks.iter().map(|t| t.size).sum();
    let stats = build_scan_stats(&tasks);
    let _ = app.emit("job-event", json!({
        "event": "scan",
        "files": tasks.len(),
        "bytes": total_bytes,
        "counts": tasks.iter().fold(HashMap::<String, u64>::new(), |mut acc, t| { *acc.entry(t.kind.clone()).or_default() += 1; acc }),
    }));

    if !check_preflight(app, &dst, total_bytes, &settings, &stats, manager) {
        return;
    }
    if !ffmpeg_ok {
        let _ = app.emit("job-event", json!({
            "event": "log",
            "message": "FFmpeg missing: video/audio/GIF files will be copied. Install it with one click in Settings > Tools.",
            "level": "warn"
        }));
    }

    let ctx = build_job_context(
        app,
        &settings,
        &tools_status,
        tasks.len(),
        total_bytes,
        manager,
        stats,
    );
    let outcomes: Arc<Mutex<Vec<Outcome>>> = Arc::new(Mutex::new(Vec::new()));
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(settings.workers.clamp(1, 32))
        .thread_name(|i| format!("shrinkwise-{i}"))
        .build();
    if let Ok(pool) = pool {
        pool.install(|| {
            tasks
                .par_iter()
                .for_each(|task| process_single_task(&ctx, task, &dst, &outcomes))
        });
    } else {
        tasks
            .iter()
            .for_each(|task| process_single_task(&ctx, task, &dst, &outcomes));
    }
    finish_job(&ctx, manager, &src, &dst, &outcomes, started);
}

pub fn load_history(limit: usize) -> Vec<serde_json::Value> {
    let Ok(text) = fs::read_to_string(portable::history_path()) else {
        return Vec::new();
    };
    let mut items: Vec<serde_json::Value> = text
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .collect();
    if items.len() > limit {
        items = items.split_off(items.len() - limit);
    }
    items
}

pub fn clear_history() -> Result<()> {
    let path = portable::history_path();
    if path.exists() {
        fs::remove_file(&path)
            .with_context(|| format!("could not clear history: {}", path.display()))?;
    }
    Ok(())
}

pub fn export_report(manager: &JobManager, path: &Path, format: &str) -> Result<()> {
    let outcomes = manager
        .last_outcomes
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    if outcomes.is_empty() {
        bail!("no completed job yet");
    }
    let path = if path.extension().is_none() {
        path.with_extension(if format == "json" { "json" } else { "csv" })
    } else {
        path.to_path_buf()
    };
    if format == "json" {
        let stats = manager
            .last_stats
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let value = json!({ "stats": stats, "files": outcomes });
        portable::atomic_write(&path, serde_json::to_string_pretty(&value)?.as_bytes())?;
    } else {
        // BOM + CRLF: Excel'in UTF-8 Turkce karakterleri dogru okumasini saglar.
        let mut csv = String::from('\u{feff}');
        csv.push_str(
            "rel,kind,status,in_bytes,out_bytes,saved_bytes,ratio,method,note,error,elapsed_ms\r\n",
        );
        for o in outcomes {
            let escape = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
            csv.push_str(&format!(
                "{},{},{},{},{},{},{:.4},{},{},{},{}\r\n",
                escape(&o.rel),
                o.kind,
                o.status,
                o.in_bytes,
                o.out_bytes,
                o.saved_bytes,
                o.ratio,
                escape(&o.method),
                escape(&o.note),
                escape(&o.error),
                o.elapsed_ms
            ));
        }
        portable::atomic_write(&path, csv.as_bytes())?;
    }
    Ok(())
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    #[cfg(windows)]
    {
        let norm = |p: &Path| {
            let text = p.to_string_lossy().replace('/', "\\");
            let text = text.strip_prefix("\\\\?\\").unwrap_or(&text);
            text.trim_end_matches('\\').to_lowercase()
        };
        norm(a) == norm(b)
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}

pub fn validate_paths(src: &Path, dst: &Path) -> Result<()> {
    if !src.is_dir() {
        bail!("source folder not found: {}", src.display());
    }
    fs::create_dir_all(dst)
        .with_context(|| format!("could not create destination folder: {}", dst.display()))?;

    // Resolve both sides before comparing so `C:\Foo`, `c:\foo`,
    // `C:\foo\..\foo` and short (8.3) paths cannot slip through.
    let src_real = fs::canonicalize(src).unwrap_or_else(|_| src.to_path_buf());
    let dst_real = fs::canonicalize(dst).unwrap_or_else(|_| dst.to_path_buf());
    if paths_equal(&src_real, &dst_real) {
        bail!("source and destination cannot be the same folder");
    }

    // Fail before a long job instead of halfway through it.
    let probe = dst.join(format!(".shrinkwise-write-{}", std::process::id()));
    match fs::write(&probe, b"ok") {
        Ok(()) => {
            let _ = fs::remove_file(&probe);
        }
        Err(err) => bail!("destination folder is not writable: {err}"),
    }
    Ok(())
}

pub fn last_stats(manager: &JobManager) -> Option<JobStats> {
    manager
        .last_stats
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

pub fn run_diagnostic(settings: &Settings, extra_dirs: &[PathBuf]) -> serde_json::Value {
    let system = crate::system::detect();
    let tools = tools::probe(settings, extra_dirs, true);
    json!({
        "system": system,
        "tools": tools,
        "paths": portable::app_paths(),
        "settings": settings,
        "elapsed": Duration::from_millis(0).as_secs(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disambiguate_keeps_extension() {
        assert_eq!(
            disambiguate(Path::new("out/clip.mp4"), 2),
            PathBuf::from("out/clip (2).mp4")
        );
        assert_eq!(
            disambiguate(Path::new("out/notes.txt.br"), 3),
            PathBuf::from("out/notes.txt (3).br")
        );
        assert_eq!(
            disambiguate(Path::new("out/README"), 2),
            PathBuf::from("out/README (2)")
        );
    }

    #[test]
    fn final_path_maps_to_same_name_for_different_sources() {
        let make = |name: &str| Task {
            src: PathBuf::from(name),
            rel: PathBuf::from(name),
            kind: "video".into(),
            size: 1,
            preferred_ext: "mp4".into(),
        };
        let a = final_path(Path::new("out"), &make("clip.mov"), "mp4", false);
        let b = final_path(Path::new("out"), &make("clip.mp4"), "mp4", false);
        assert_eq!(
            a, b,
            "both map to the same output name and need disambiguation"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_paths_compare_case_insensitively() {
        assert!(paths_equal(Path::new("C:\\Out"), Path::new("c:\\out\\")));
        assert!(paths_equal(
            Path::new("\\\\?\\C:\\Out"),
            Path::new("C:\\out")
        ));
        assert!(!paths_equal(Path::new("C:\\out"), Path::new("C:\\other")));
    }
}
