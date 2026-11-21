use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::portable;

/// Kullanıcı ayarları. Tüm alanlar `#[serde(default)]` sayesinde eksik/eski
/// `settings.json` dosyalarıyla geriye dönük uyumludur.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    // ---- genel
    pub lossy: u8,
    pub lossless_mode: bool,
    pub simple_mode: bool,
    pub workers: usize,
    pub keep_larger: bool,
    pub keep_metadata: bool,
    pub keep_mtime: bool,
    pub min_size_kb: u64,
    pub limit_mb: u64,
    pub exclude_globs: String,
    pub only_kinds: Vec<String>,
    pub only_strict: bool,
    pub dry_run: bool,
    pub verify: bool,
    pub deep_image: bool,

    // ---- arayüz
    pub theme: String,
    pub font_scale: f32,
    pub language: String,
    pub show_file_list: bool,
    pub notify_done: bool,
    pub auto_check_tools: bool,

    // ---- görsel
    pub image_format: String,
    pub convert_png_to_jpeg: bool,
    pub convert_bmp_to_jpeg: bool,
    pub image_max_mp: f32,
    pub image_quality: f32,
    pub image_max_side_pct: u32,
    pub image_alpha: String,
    pub extra_webp_too: bool,

    // ---- animasyon
    pub gif_mode: String,
    pub gif_max_fps: String,
    pub gif_max_width: u32,
    pub gif_loop: bool,

    // ---- video
    pub video_codec: String,
    pub encoder_prefer: String,
    pub video_resolution: String,
    pub video_max_fps: String,
    pub video_audio_codec: String,
    pub video_scale_pct: u32,
    pub video_10bit: bool,
    pub video_scale_search: String,
    pub video_two_pass: String,
    pub video_tune: String,
    pub video_denoise: bool,
    pub video_hdr_tonemap: bool,
    pub video_faststart: bool,
    pub video_keep_subs: bool,
    pub video_deep_verify: bool,

    // ---- ses
    pub audio_normalize: bool,
    pub audio_mono: bool,
    pub audio_trim_silence: bool,

    // ---- belgeler
    pub pdf_mode: String,
    pub pdf_raster_dpi: u32,
    pub office_mode: String,
    pub archive_mode: String,
    pub text_mode: String,
    pub svg_mode: String,
    pub font_mode: String,
    pub zip_deep: bool,
    pub zip_deep_depth: u32,

    // ---- hedef boyut
    pub target_mode: String,
    pub target_image_kb: f64,
    pub target_video_mb: f64,
    pub target_audio_kb: f64,
    pub target_gif_kb: f64,
    pub target_pdf_kb: f64,
    pub target_total_mb: f64,
    pub target_min_quality: u8,

    // ---- akıllı mod
    pub smart_mode: bool,
    pub smart_min_ssim: f64,
    pub smart_max_encodes: u32,
    pub smart_scale_search: bool,
    pub smart_content_analysis: bool,
    pub smart_gif_race: bool,
    pub smart_audio_guard: bool,

    // ---- GPU / araçlar
    pub gpu: bool,
    pub gpu_decoder: String,
    pub gpu_device: u32,
    pub gpu_av1: bool,
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
    pub seven_zip_path: String,
    pub ghostscript_path: String,
    pub qpdf_path: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            lossy: 50,
            lossless_mode: false,
            simple_mode: false,
            workers: std::thread::available_parallelism()
                .map(|n| n.get().clamp(1, 8))
                .unwrap_or(4),
            keep_larger: true,
            keep_metadata: true,
            keep_mtime: true,
            min_size_kb: 0,
            limit_mb: 0,
            exclude_globs: String::new(),
            only_kinds: Vec::new(),
            only_strict: false,
            dry_run: false,
            verify: true,
            deep_image: true,

            theme: "dark".into(),
            font_scale: 1.0,
            language: "en".into(),
            show_file_list: true,
            notify_done: true,
            auto_check_tools: true,

            image_format: "auto".into(),
            convert_png_to_jpeg: false,
            convert_bmp_to_jpeg: true,
            image_max_mp: 0.0,
            image_quality: 0.0,
            image_max_side_pct: 100,
            image_alpha: "auto".into(),
            extra_webp_too: true,

            gif_mode: "auto".into(),
            gif_max_fps: "auto".into(),
            gif_max_width: 0,
            gif_loop: true,

            video_codec: "auto".into(),
            encoder_prefer: "quality".into(),
            video_resolution: "keep".into(),
            video_max_fps: "keep".into(),
            video_audio_codec: "auto".into(),
            video_scale_pct: 100,
            video_10bit: true,
            video_scale_search: "auto".into(),
            video_two_pass: "auto".into(),
            video_tune: "auto".into(),
            video_denoise: false,
            video_hdr_tonemap: true,
            video_faststart: true,
            video_keep_subs: true,
            video_deep_verify: true,

            audio_normalize: false,
            audio_mono: false,
            audio_trim_silence: false,

            pdf_mode: "max".into(),
            pdf_raster_dpi: 0,
            office_mode: "members".into(),
            archive_mode: "max".into(),
            text_mode: "auto".into(),
            svg_mode: "max".into(),
            font_mode: "max".into(),
            zip_deep: true,
            zip_deep_depth: 2,

            target_mode: "off".into(),
            target_image_kb: 0.0,
            target_video_mb: 0.0,
            target_audio_kb: 0.0,
            target_gif_kb: 0.0,
            target_pdf_kb: 0.0,
            target_total_mb: 0.0,
            target_min_quality: 18,

            smart_mode: false,
            smart_min_ssim: 0.0,
            smart_max_encodes: 14,
            smart_scale_search: true,
            smart_content_analysis: true,
            smart_gif_race: true,
            smart_audio_guard: true,

            gpu: true,
            gpu_decoder: "auto".into(),
            gpu_device: 0,
            gpu_av1: false,
            ffmpeg_path: String::new(),
            ffprobe_path: String::new(),
            seven_zip_path: String::new(),
            ghostscript_path: String::new(),
            qpdf_path: String::new(),
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let path = portable::settings_path();
        match fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Settings>(&text) {
                Ok(mut settings) => {
                    settings.sanitize();
                    settings
                }
                Err(_) => Self::default(),
            },
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = portable::settings_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("folder: {}", parent.display()))?;
        }
        let text = serde_json::to_string_pretty(self)?;
        portable::atomic_write(&path, text.as_bytes())
            .with_context(|| format!("could not write settings: {}", path.display()))?;
        Ok(())
    }

    pub fn sanitize(&mut self) {
        self.lossy = self.lossy.min(100);
        self.workers = self.workers.clamp(1, 32);
        self.font_scale = self.font_scale.clamp(0.8, 1.6);
        if self.theme != "light" {
            self.theme = "dark".into();
        }
        self.image_max_side_pct = self.image_max_side_pct.clamp(10, 100);
        self.video_scale_pct = self.video_scale_pct.clamp(25, 100);
        self.image_max_mp = self.image_max_mp.max(0.0);
        self.image_quality = self.image_quality.clamp(0.0, 100.0);
        self.gif_max_width = self.gif_max_width.min(7680);
        self.smart_max_encodes = self.smart_max_encodes.clamp(1, 40);
        self.zip_deep_depth = self.zip_deep_depth.min(4);
        for v in [
            &mut self.target_image_kb,
            &mut self.target_video_mb,
            &mut self.target_audio_kb,
            &mut self.target_gif_kb,
            &mut self.target_pdf_kb,
            &mut self.target_total_mb,
        ] {
            if !v.is_finite() || *v < 0.0 {
                *v = 0.0;
            }
        }
        self.only_kinds.retain(|k| !k.trim().is_empty());
        self.only_kinds.sort();
        self.only_kinds.dedup();
    }

    /// Kayıplılık yüzdesi -> 0.0..1.0
    pub fn t(&self) -> f32 {
        self.lossy.min(100) as f32 / 100.0
    }

    pub fn video_crf(&self) -> u32 {
        (15.0 + 25.0 * self.t().powf(1.15)).round() as u32
    }

    pub fn image_quality(&self) -> u8 {
        if self.image_quality > 0.0 {
            return self.image_quality.round().clamp(1.0, 100.0) as u8;
        }
        (96.0 - 84.0 * self.t()).round().clamp(8.0, 98.0) as u8
    }

    pub fn music_kbps(&self) -> u32 {
        let raw = 256.0 - 188.0 * self.t();
        const STEPS: [u32; 14] = [
            320, 256, 224, 192, 160, 128, 112, 96, 80, 64, 56, 48, 40, 32,
        ];
        *STEPS
            .iter()
            .min_by(|a, b| {
                (**a as f32 - raw)
                    .abs()
                    .partial_cmp(&(**b as f32 - raw).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(&160)
    }

    pub fn video_audio_kbps(&self) -> u32 {
        (self.music_kbps() as f32 * 1.15) as u32
    }

    pub fn image_max_side_factor(&self) -> f32 {
        (1.0 - 0.67 * self.t()).max(0.25)
    }

    pub fn gif_fps(&self) -> u32 {
        let raw = 20.0 - 14.0 * self.t();
        raw.round().max(4.0) as u32
    }

    pub fn gif_width(&self) -> u32 {
        if self.gif_max_width > 0 {
            return self.gif_max_width;
        }
        let raw = 640.0 - 320.0 * self.t();
        ((raw / 2.0).round() * 2.0).max(160.0) as u32
    }

    pub fn video_height_limit(&self) -> Option<u32> {
        match self.video_resolution.as_str() {
            "2160p" => Some(2160),
            "1440p" => Some(1440),
            "1080p" => Some(1080),
            "720p" => Some(720),
            "480p" => Some(480),
            _ => None,
        }
    }

    pub fn video_fps_limit(&self) -> Option<u32> {
        match self.video_max_fps.as_str() {
            "keep" | "auto" | "" => None,
            value => value.parse::<u32>().ok(),
        }
    }

    pub fn settings_path_string() -> String {
        portable::settings_path().display().to_string()
    }
}

pub fn save_to(path: &Path, settings: &Settings) -> Result<()> {
    let text = serde_json::to_string_pretty(settings)?;
    portable::atomic_write(path, text.as_bytes())
}
