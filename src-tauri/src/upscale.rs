use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use image::DynamicImage;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use serde::Serialize;

use crate::ffmpeg_engine::{self, VideoUpscale};
use crate::image_engine;
use crate::portable;

static CANCEL: AtomicBool = AtomicBool::new(false);
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

const MAX_LANCZOS_PIXELS: u64 = 120_000_000;

#[derive(Debug, Clone, Default)]
pub struct EnginePaths {
    pub ffmpeg: Option<PathBuf>,
    pub ffprobe: Option<PathBuf>,
    pub realesrgan: Option<PathBuf>,
    pub waifu2x: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub bytes: u64,
    pub preview: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpscaleResult {
    pub output: String,
    pub engine: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub elapsed_ms: u64,
    pub before_preview: String,
    pub after_preview: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpscaleProgress {
    pub phase: String,
    pub percent: f64,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Engine {
    RealEsrgan,
    Waifu2x,
    Lanczos,
}

impl Engine {
    fn label(self) -> &'static str {
        match self {
            Engine::RealEsrgan => "Real-ESRGAN",
            Engine::Waifu2x => "Waifu2x",
            Engine::Lanczos => "Lanczos",
        }
    }
}

pub fn request_cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}

const IMAGE_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff", "ico", "qoi", "tga", "dds", "pnm",
    "avif", "heic", "heif", "jxl", "jp2", "j2k", "hdr", "exr", "raw", "cr2", "cr3", "nef", "arw",
    "dng", "orf", "rw2", "raf", "sr2", "pef", "srw",
];

const VIDEO_EXTS: &[&str] = &[
    "mp4", "mkv", "mov", "avi", "webm", "m4v", "wmv", "flv", "mpg", "mpeg", "ts", "m2ts", "3gp",
    "ogv",
];

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase()
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("output")
        .to_string()
}

fn output_dir() -> Result<PathBuf> {
    let dir = portable::app_dir().join("Upscaled");
    fs::create_dir_all(&dir)
        .with_context(|| format!("could not create output folder: {}", dir.display()))?;
    Ok(dir)
}

pub fn inspect(input: &Path, paths: &EnginePaths) -> Result<MediaInfo> {
    let metadata =
        fs::metadata(input).with_context(|| format!("could not open file: {}", input.display()))?;
    if !metadata.is_file() {
        bail!("the selected path is not a file");
    }
    let ext = extension(input);
    let is_video = VIDEO_EXTS.contains(&ext.as_str());
    let is_image = IMAGE_EXTS.contains(&ext.as_str());

    if is_video || (!is_image && paths.ffprobe.is_some()) {
        if let Some(ffprobe) = paths.ffprobe.as_deref()
            && let Ok(probe) = ffmpeg_engine::probe(ffprobe, input)
            && probe.has_video
        {
            let preview = paths
                .ffmpeg
                .as_deref()
                .and_then(|ffmpeg| video_poster(ffmpeg, input, probe.duration))
                .unwrap_or_default();
            return Ok(MediaInfo {
                path: input.display().to_string(),
                name: file_name(input),
                kind: "video".into(),
                width: probe.width,
                height: probe.height,
                duration: probe.duration,
                bytes: metadata.len(),
                preview,
            });
        }
        if is_video {
            bail!("could not read video info; make sure FFmpeg is installed");
        }
    }

    let (image, temp) = image_engine::decode(input, paths.ffmpeg.as_deref())?;
    let preview = preview_data_url(&image, 1200);
    let info = MediaInfo {
        path: input.display().to_string(),
        name: file_name(input),
        kind: "image".into(),
        width: image.width(),
        height: image.height(),
        duration: 0.0,
        bytes: metadata.len(),
        preview,
    };
    if let Some(temp) = temp {
        let _ = fs::remove_file(temp);
    }
    Ok(info)
}

pub fn upscale_image(
    input: &Path,
    scale: u32,
    recipe: &str,
    model: &str,
    paths: &EnginePaths,
    progress: &(dyn Fn(&str, f64, &str) + Sync),
) -> Result<UpscaleResult> {
    CANCEL.store(false, Ordering::SeqCst);
    let started = Instant::now();
    let scale = if scale >= 4 { 4 } else { 2 };

    progress("analyze", 4.0, "Reading image…");
    let (before, decode_temp) = image_engine::decode(input, paths.ffmpeg.as_deref())?;
    if let Some(temp) = decode_temp {
        let _ = fs::remove_file(temp);
    }
    let before_preview = preview_data_url(&before, 1200);
    let (src_w, src_h) = (before.width(), before.height());

    let engine = match recipe {
        "realesrgan" => Engine::RealEsrgan,
        "waifu2x" => Engine::Waifu2x,
        "lanczos" => Engine::Lanczos,
        _ => {
            if paths.realesrgan.is_some() {
                Engine::RealEsrgan
            } else if scale == 2 && paths.waifu2x.is_some() {
                Engine::Waifu2x
            } else {
                Engine::Lanczos
            }
        }
    };

    if engine == Engine::RealEsrgan && paths.realesrgan.is_none() {
        bail!("Real-ESRGAN is not installed; add it from Settings › Tools");
    }
    if engine == Engine::Waifu2x {
        if paths.waifu2x.is_none() {
            bail!("Waifu2x is not installed; add it from Settings › Tools");
        }
        if scale != 2 {
            bail!("Waifu2x only supports 2x; choose 2x or use Real-ESRGAN for 4x");
        }
    }

    let dir = output_dir()?;
    let suffix = match engine {
        Engine::RealEsrgan => "realesrgan",
        Engine::Waifu2x => "waifu2x",
        Engine::Lanczos => "lanczos",
    };
    let output = dir.join(format!("{}_upscaled_{scale}x_{suffix}.png", stem(input)));

    let note = match engine {
        Engine::RealEsrgan => {
            let exe = paths
                .realesrgan
                .as_deref()
                .ok_or_else(|| anyhow!("Real-ESRGAN is not installed"))?;
            let model_name = if model == "anime" {
                "realesrgan-x4plus-anime"
            } else {
                "realesrgan-x4plus"
            };
            progress("upscale", 18.0, "Running Real-ESRGAN…");
            let (source, temp) = esrgan_source(input, &before)?;
            let args = vec![
                "-i".to_string(),
                source.display().to_string(),
                "-o".to_string(),
                output.display().to_string(),
                "-s".to_string(),
                scale.to_string(),
                "-n".to_string(),
                model_name.to_string(),
                "-f".to_string(),
                "png".to_string(),
            ];
            let result = run_tool(exe, &args, Duration::from_secs(60 * 30));
            if let Some(temp) = temp {
                let _ = fs::remove_file(temp);
            }
            if let Err(err) = result {
                let _ = fs::remove_file(&output);
                return Err(err);
            }
            format!("Real-ESRGAN · {model_name}")
        }
        Engine::Waifu2x => {
            let exe = paths
                .waifu2x
                .as_deref()
                .ok_or_else(|| anyhow!("Waifu2x is not installed"))?;
            progress("upscale", 18.0, "Running Waifu2x…");
            let (source, temp) = esrgan_source(input, &before)?;
            let args = vec![
                "-i".to_string(),
                source.display().to_string(),
                "-o".to_string(),
                output.display().to_string(),
                "-s".to_string(),
                "2".to_string(),
                "-n".to_string(),
                "1".to_string(),
                "-f".to_string(),
                "png".to_string(),
            ];
            let result = run_tool(exe, &args, Duration::from_secs(60 * 30));
            if let Some(temp) = temp {
                let _ = fs::remove_file(temp);
            }
            if let Err(err) = result {
                let _ = fs::remove_file(&output);
                return Err(err);
            }
            "Waifu2x · 2x".to_string()
        }
        Engine::Lanczos => {
            let pixels = src_w as u64 * src_h as u64 * scale as u64 * scale as u64;
            if pixels > MAX_LANCZOS_PIXELS {
                bail!("image is too large for a {scale}x upscale; try 2x or a smaller source");
            }
            progress("upscale", 30.0, "Resampling with Lanczos…");
            let resized = before.resize_exact(src_w * scale, src_h * scale, FilterType::Lanczos3);
            resized
                .save(&output)
                .with_context(|| format!("could not write {}", output.display()))?;
            "Lanczos high quality".to_string()
        }
    };

    if CANCEL.load(Ordering::SeqCst) {
        let _ = fs::remove_file(&output);
        bail!("upscaling cancelled");
    }
    if !output.is_file() {
        bail!("{} did not produce an output file", engine.label());
    }

    progress("preview", 92.0, "Preparing preview…");
    let (width, height) =
        image::image_dimensions(&output).unwrap_or((src_w * scale, src_h * scale));
    let after_preview = image::open(&output)
        .map(|image| preview_data_url(&image, 1200))
        .unwrap_or_default();
    let bytes = fs::metadata(&output).map(|meta| meta.len()).unwrap_or(0);
    progress("done", 100.0, "Upscale complete");

    Ok(UpscaleResult {
        output: output.display().to_string(),
        engine: engine.label().into(),
        width,
        height,
        bytes,
        elapsed_ms: started.elapsed().as_millis() as u64,
        before_preview,
        after_preview,
        note,
    })
}

pub fn upscale_video(
    input: &Path,
    target: &str,
    quality: &str,
    paths: &EnginePaths,
    progress: &(dyn Fn(&str, f64, &str) + Sync),
) -> Result<UpscaleResult> {
    CANCEL.store(false, Ordering::SeqCst);
    let started = Instant::now();
    let ffmpeg = paths
        .ffmpeg
        .as_deref()
        .ok_or_else(|| anyhow!("FFmpeg is not installed; add it from Settings › Tools"))?;
    let ffprobe = paths
        .ffprobe
        .as_deref()
        .ok_or_else(|| anyhow!("FFprobe is not installed; add FFmpeg from Settings › Tools"))?;

    progress("analyze", 2.0, "Reading video…");
    let probe = ffmpeg_engine::probe(ffprobe, input)?;
    if !probe.has_video {
        bail!("no video stream found");
    }

    let (height, factor, label) = match target {
        "720p" => (Some(720), None, "720p"),
        "1080p" => (Some(1080), None, "1080p"),
        "1440p" => (Some(1440), None, "1440p"),
        "2160p" => (Some(2160), None, "4K"),
        "2x" => (None, Some(2), "2x"),
        "4x" => (None, Some(4), "4x"),
        other => bail!("unknown target resolution: {other}"),
    };
    let (crf, preset) = if quality == "quality" {
        (17, "slow")
    } else {
        (20, "medium")
    };

    let dir = output_dir()?;
    let output = dir.join(format!(
        "{}_upscaled_{}.mp4",
        stem(input),
        label.to_lowercase().replace(' ', "")
    ));
    let opts = VideoUpscale {
        target_height: height,
        factor,
        crf,
        preset,
    };

    let report = |fraction: f32| {
        let percent = (fraction as f64 * 96.0).clamp(1.0, 96.0);
        progress("encode", percent, "Encoding frames…");
    };
    progress("encode", 4.0, "Starting encoder…");
    if let Err(err) =
        ffmpeg_engine::upscale_video(input, &output, &opts, ffmpeg, ffprobe, &CANCEL, &report)
    {
        let _ = fs::remove_file(&output);
        return Err(err);
    }

    if CANCEL.load(Ordering::SeqCst) {
        let _ = fs::remove_file(&output);
        bail!("upscaling cancelled");
    }
    if !output.is_file() {
        bail!("FFmpeg did not produce an output file");
    }

    progress("finalize", 98.0, "Finalizing…");
    let out_probe = ffmpeg_engine::probe(ffprobe, &output).unwrap_or_default();
    let bytes = fs::metadata(&output).map(|meta| meta.len()).unwrap_or(0);
    progress("done", 100.0, "Upscale complete");

    Ok(UpscaleResult {
        output: output.display().to_string(),
        engine: format!("FFmpeg Lanczos · {label}"),
        width: out_probe.width,
        height: out_probe.height,
        bytes,
        elapsed_ms: started.elapsed().as_millis() as u64,
        before_preview: String::new(),
        after_preview: String::new(),
        note: if quality == "quality" {
            "H.264 CRF 17 · slow preset".into()
        } else {
            "H.264 CRF 20 · medium preset".into()
        },
    })
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file")
        .to_string()
}

fn preview_data_url(image: &DynamicImage, max_side: u32) -> String {
    let thumb = image.thumbnail(max_side, max_side);
    let rgb = thumb.to_rgb8();
    let mut buffer = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut buffer, 84);
    if encoder.encode_image(&rgb).is_err() {
        return String::new();
    }
    format!("data:image/jpeg;base64,{}", base64(&buffer))
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        let value = (u32::from(first) << 16) | (u32::from(second) << 8) | u32::from(third);
        out.push(TABLE[((value >> 18) & 63) as usize] as char);
        out.push(TABLE[((value >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

fn video_poster(ffmpeg: &Path, input: &Path, duration: f64) -> Option<String> {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let out = portable::temp_dir().join(format!("sw-poster-{}-{counter}.jpg", std::process::id()));
    let seek = if duration > 1.5 {
        (duration * 0.1).min(3.0)
    } else {
        0.0
    };
    let args = vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-nostdin".to_string(),
        "-y".to_string(),
        "-ss".to_string(),
        format!("{seek:.3}"),
        "-i".to_string(),
        input.display().to_string(),
        "-frames:v".to_string(),
        "1".to_string(),
        "-vf".to_string(),
        "scale=960:-2".to_string(),
        out.display().to_string(),
    ];
    let ok = portable::run_capture(ffmpeg, &args, Duration::from_secs(30))
        .map(|result| result.status.success())
        .unwrap_or(false);
    let data = if ok { fs::read(&out).ok() } else { None };
    let _ = fs::remove_file(&out);
    data.map(|bytes| format!("data:image/jpeg;base64,{}", base64(&bytes)))
}

fn esrgan_source(input: &Path, decoded: &DynamicImage) -> Result<(PathBuf, Option<PathBuf>)> {
    let ext = extension(input);
    if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "bmp") {
        return Ok((input.to_path_buf(), None));
    }
    let temp = portable::temp_dir().join(format!(
        "sw-esrgan-{}-{}.png",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    decoded
        .save_with_format(&temp, image::ImageFormat::Png)
        .with_context(|| {
            format!(
                "could not prepare input for Real-ESRGAN: {}",
                temp.display()
            )
        })?;
    Ok((temp.clone(), Some(temp)))
}

fn run_tool(program: &Path, args: &[String], timeout: Duration) -> Result<()> {
    use std::io::Read;

    let mut command = portable::new_command(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if let Some(dir) = program.parent() {
        command.current_dir(dir);
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("could not start {}", program.display()))?;

    let stderr = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(mut pipe) = stderr {
            let _ = pipe.read_to_string(&mut text);
        }
        text
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        if CANCEL.load(Ordering::SeqCst) {
            portable::kill_child(&mut child);
            let _ = reader.join();
            bail!("upscaling cancelled");
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            portable::kill_child(&mut child);
            let _ = reader.join();
            bail!(
                "upscaling timed out after {} minutes",
                timeout.as_secs() / 60
            );
        }
        std::thread::sleep(Duration::from_millis(120));
    };

    let stderr = reader.join().unwrap_or_default();
    if status.success() {
        return Ok(());
    }
    let tail: Vec<&str> = stderr
        .lines()
        .filter(|line| !line.trim().is_empty())
        .rev()
        .take(4)
        .collect();
    bail!(
        "{} failed (code {:?}): {}",
        program.display(),
        status.code(),
        tail.into_iter().rev().collect::<Vec<_>>().join(" | ")
    )
}
