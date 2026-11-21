// Bazi yardimci motor API'leri (PDF/ofis/arsiv, teshis, AVIF) sonraki surumde
// arayuze baglanacak; simdilik derleme uyarisi uretmemeleri icin izin veriyoruz.
#![allow(dead_code)]

mod classifier;
mod commands;
mod compress;
mod container_engine;
mod ffmpeg_engine;
mod image_engine;
mod pdf_engine;
mod portable;
mod settings;
mod system;
mod text_engine;
mod tools;
mod upscale;

use compress::JobManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = portable::ensure_dirs();
    let _ = portable::install_job_object();
    portable::configure_webview2();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(JobManager::default())
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::save_settings,
            commands::refresh_system,
            commands::refresh_tools,
            commands::refresh_tool,
            commands::detect_encoders,
            commands::install_tool,
            commands::benchmark_cpu,
            commands::inspect_media,
            commands::upscale_image,
            commands::upscale_video,
            commands::cancel_upscale,
            commands::scan_source,
            commands::start_job,
            commands::cancel_job,
            commands::job_status,
            commands::export_report,
            commands::job_history,
            commands::clear_history,
            commands::resolve_source,
            commands::get_paths,
            commands::reset_settings,
            commands::flush_settings,
        ])
        .setup(|_app| {
            let _ = portable::ensure_dirs();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("ShrinkWise failed to start");
}
