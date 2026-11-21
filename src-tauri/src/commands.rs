use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::compress::{self, JobManager, JobStats};
use crate::settings::Settings;
use crate::system::{self, CpuBenchmark, SystemInfo};
use crate::tools::{self, EncoderSupport, ToolInfo, ToolsStatus};
use crate::upscale;
use crate::{classifier, portable};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub version: String,
    pub settings: Settings,
    pub system: SystemInfo,
    pub tools: ToolsStatus,
    pub paths: serde_json::Value,
    pub settings_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartResponse {
    pub started: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStatus {
    pub running: bool,
    pub stats: Option<JobStats>,
}

fn extra_tool_dirs(app: &AppHandle) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(resource) = app.path().resource_dir() {
        dirs.push(resource.join("tools").join("bin"));
        dirs.push(resource.join("tools"));
    }
    dirs
}

fn map_err(err: impl std::fmt::Display) -> String {
    err.to_string()
}

#[tauri::command]
pub async fn bootstrap(app: AppHandle) -> Result<Bootstrap, String> {
    let extra = extra_tool_dirs(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let settings = Settings::load();
        let system = system::detect();
        let tools = tools::probe(&settings, &extra, settings.auto_check_tools);
        Bootstrap {
            version: env!("CARGO_PKG_VERSION").into(),
            settings,
            system,
            tools,
            paths: portable::app_paths(),
            settings_path: Settings::settings_path_string(),
        }
    })
    .await
    .map_err(map_err)
}

#[tauri::command]
pub async fn save_settings(settings: Settings) -> Result<Settings, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut settings = settings;
        settings.sanitize();
        settings.save().map_err(|e| e.to_string())?;
        Ok(settings)
    })
    .await
    .map_err(map_err)?
}

#[tauri::command]
pub async fn refresh_system() -> Result<SystemInfo, String> {
    tauri::async_runtime::spawn_blocking(system::detect)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn refresh_tools(app: AppHandle, settings: Settings) -> Result<ToolsStatus, String> {
    let extra = extra_tool_dirs(&app);
    tauri::async_runtime::spawn_blocking(move || tools::probe(&settings, &extra, true))
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn refresh_tool(
    app: AppHandle,
    settings: Settings,
    id: String,
) -> Result<ToolInfo, String> {
    let extra = extra_tool_dirs(&app);
    tauri::async_runtime::spawn_blocking(move || tools::probe_single(&settings, &extra, &id))
        .await
        .map_err(map_err)?
        .map_err(map_err)
}

#[tauri::command]
pub async fn detect_encoders(app: AppHandle, settings: Settings) -> Result<EncoderSupport, String> {
    let extra = extra_tool_dirs(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let status = tools::probe(&settings, &extra, true);
        let ffmpeg = status
            .ffmpeg
            .path
            .as_ref()
            .map(PathBuf::from)
            .ok_or_else(|| "FFmpeg not found".to_string())?;
        let sys = system::detect();
        let vendor = sys
            .gpus
            .iter()
            .find(|gpu| !gpu.software)
            .map(|gpu| gpu.vendor.as_str())
            .unwrap_or("cpu");
        Ok::<_, String>(tools::detect_hardware_encoders(
            &ffmpeg,
            &status.encoders,
            vendor,
        ))
    })
    .await
    .map_err(map_err)?
}

#[tauri::command]
pub async fn install_tool(app: AppHandle, id: String) -> Result<ToolInfo, String> {
    let result = tools::install_tool(&app, &id).await.map_err(map_err)?;
    let _ = app.emit("tools-changed", &result);
    Ok(result)
}

fn engine_paths(app: &AppHandle, settings: &Settings) -> upscale::EnginePaths {
    let extra = extra_tool_dirs(app);
    let status = tools::probe(settings, &extra, false);
    let path = |info: &ToolInfo| info.path.as_ref().map(PathBuf::from);
    upscale::EnginePaths {
        ffmpeg: path(&status.ffmpeg),
        ffprobe: path(&status.ffprobe),
        realesrgan: path(&status.realesrgan),
        waifu2x: path(&status.waifu2x),
    }
}

#[tauri::command]
pub async fn inspect_media(
    app: AppHandle,
    settings: Settings,
    path: String,
) -> Result<upscale::MediaInfo, String> {
    let paths = engine_paths(&app, &settings);
    tauri::async_runtime::spawn_blocking(move || {
        upscale::inspect(Path::new(&path), &paths).map_err(map_err)
    })
    .await
    .map_err(map_err)?
}

#[tauri::command]
pub async fn upscale_image(
    app: AppHandle,
    settings: Settings,
    path: String,
    scale: u32,
    recipe: String,
    model: String,
) -> Result<upscale::UpscaleResult, String> {
    let paths = engine_paths(&app, &settings);
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let progress = |phase: &str, percent: f64, message: &str| {
            let _ = handle.emit(
                "upscale-progress",
                upscale::UpscaleProgress {
                    phase: phase.into(),
                    percent,
                    message: message.into(),
                },
            );
        };
        upscale::upscale_image(Path::new(&path), scale, &recipe, &model, &paths, &progress)
            .map_err(map_err)
    })
    .await
    .map_err(map_err)?
}

#[tauri::command]
pub async fn upscale_video(
    app: AppHandle,
    settings: Settings,
    path: String,
    target: String,
    quality: String,
) -> Result<upscale::UpscaleResult, String> {
    let paths = engine_paths(&app, &settings);
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let progress = |phase: &str, percent: f64, message: &str| {
            let _ = handle.emit(
                "upscale-progress",
                upscale::UpscaleProgress {
                    phase: phase.into(),
                    percent,
                    message: message.into(),
                },
            );
        };
        upscale::upscale_video(Path::new(&path), &target, &quality, &paths, &progress)
            .map_err(map_err)
    })
    .await
    .map_err(map_err)?
}

#[tauri::command]
pub fn cancel_upscale() {
    upscale::request_cancel();
}

#[tauri::command]
pub async fn benchmark_cpu() -> Result<CpuBenchmark, String> {
    tauri::async_runtime::spawn_blocking(system::benchmark_cpu)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn scan_source(
    app: AppHandle,
    src: String,
    dst: String,
    settings: Settings,
) -> Result<classifier::ScanResult, String> {
    let extra = extra_tool_dirs(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let src = PathBuf::from(src);
        let dst = PathBuf::from(dst);
        if !src.is_dir() {
            return Err("source folder not found".to_string());
        }
        let status = tools::probe(&settings, &extra, false);
        classifier::scan(&src, &dst, &settings, status.ffmpeg.available).map_err(map_err)
    })
    .await
    .map_err(map_err)?
}

fn begin_job(manager: &JobManager) -> Result<(), String> {
    if manager
        .running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("a job is already running".into());
    }
    manager.cancel.store(false, Ordering::SeqCst);
    manager
        .last_outcomes
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
    *manager.last_stats.lock().unwrap_or_else(|e| e.into_inner()) = None;
    Ok(())
}

#[tauri::command]
pub async fn start_job(
    app: AppHandle,
    src: String,
    dst: String,
    settings: Settings,
    only: Option<Vec<String>>,
) -> Result<StartResponse, String> {
    let src_path = PathBuf::from(src);
    let dst_path = PathBuf::from(dst);
    compress::validate_paths(&src_path, &dst_path).map_err(map_err)?;

    let mut settings = settings;
    settings.sanitize();
    settings.save().map_err(map_err)?;
    let extra = extra_tool_dirs(&app);

    begin_job(&app.state::<JobManager>())?;

    let app_for_job = app.clone();
    let settings_for_job = settings.clone();
    let src_for_job = src_path.clone();
    let dst_for_job = dst_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let manager = app_for_job.state::<JobManager>();
        compress::run_job(
            &app_for_job,
            src_for_job,
            dst_for_job,
            settings_for_job,
            extra,
            &manager,
            only,
        );
    });

    Ok(StartResponse {
        started: true,
        message: format!("{} -> {}", src_path.display(), dst_path.display()),
    })
}

#[tauri::command]
pub async fn cancel_job(app: AppHandle) -> Result<bool, String> {
    let manager = app.state::<JobManager>();
    let running: &AtomicBool = &manager.running;
    if !running.load(Ordering::SeqCst) {
        return Ok(false);
    }
    manager.cancel.store(true, Ordering::SeqCst);
    Ok(true)
}

#[tauri::command]
pub async fn job_status(app: AppHandle) -> Result<JobStatus, String> {
    let manager = app.state::<JobManager>();
    Ok(JobStatus {
        running: manager.running.load(Ordering::SeqCst),
        stats: compress::last_stats(&manager),
    })
}

#[tauri::command]
pub async fn job_history(limit: Option<usize>) -> Result<Vec<serde_json::Value>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        Ok(compress::load_history(limit.unwrap_or(12).clamp(1, 100)))
    })
    .await
    .map_err(map_err)?
}

#[tauri::command]
pub async fn clear_history() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| compress::clear_history().map_err(|e| e.to_string()))
        .await
        .map_err(map_err)?
}

/// Turns a dropped file or folder into a source folder path.
#[tauri::command]
pub fn resolve_source(path: String) -> Result<String, String> {
    let path = PathBuf::from(path);
    if path.is_dir() {
        return Ok(path.display().to_string());
    }
    if path.is_file() {
        return path
            .parent()
            .map(|parent| parent.display().to_string())
            .ok_or_else(|| "could not resolve the folder of the dropped file".to_string());
    }
    Err("the dropped path could not be found".into())
}

#[tauri::command]
pub async fn export_report(app: AppHandle, path: String, format: String) -> Result<String, String> {
    let manager = app.state::<JobManager>();
    let path_buf = PathBuf::from(&path);
    compress::export_report(&manager, &path_buf, &format).map_err(map_err)?;
    Ok(path_buf.display().to_string())
}

#[tauri::command]
pub fn get_paths() -> serde_json::Value {
    portable::app_paths()
}

#[tauri::command]
pub async fn reset_settings() -> Result<Settings, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let settings = Settings::default();
        settings.save().map_err(|e| e.to_string())?;
        Ok(settings)
    })
    .await
    .map_err(map_err)?
}

#[tauri::command]
pub async fn flush_settings(
    app: AppHandle,
    _state: State<'_, JobManager>,
    settings: Settings,
) -> Result<(), String> {
    let _ = app;
    tauri::async_runtime::spawn_blocking(move || settings.save().map_err(|e| e.to_string()))
        .await
        .map_err(map_err)?
}
