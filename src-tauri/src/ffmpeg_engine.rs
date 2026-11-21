use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;

use crate::portable;
use crate::settings::Settings;

static FF_COUNTER: AtomicU64 = AtomicU64::new(1);

pub struct FfmpegEnv<'a> {
    pub ffmpeg: &'a Path,
    pub ffprobe: &'a Path,
    pub encoders: &'a [String],
    pub cancel: &'a AtomicBool,
}

#[derive(Debug, Clone, Default)]
pub struct MediaProbe {
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub vcodec: String,
    pub acodec: String,
    pub has_video: bool,
    pub has_audio: bool,
}

fn parse_streams(value: &Value, info: &mut MediaProbe) {
    let Some(streams) = value.get("streams").and_then(|s| s.as_array()) else {
        return;
    };
    for stream in streams {
        match stream.get("codec_type").and_then(|c| c.as_str()) {
            Some("video") => {
                info.has_video = true;
                info.width = stream.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                info.height = stream.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                info.vcodec = stream
                    .get("codec_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
            }
            Some("audio") => {
                info.has_audio = true;
                info.acodec = stream
                    .get("codec_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
            }
            _ => {}
        }
    }
}

pub fn probe(ffprobe: &Path, input: &Path) -> Result<MediaProbe> {
    let args = vec![
        "-v".to_string(),
        "error".to_string(),
        "-print_format".to_string(),
        "json".to_string(),
        "-show_format".to_string(),
        "-show_streams".to_string(),
        input.display().to_string(),
    ];
    let out = portable::run_capture(ffprobe, &args, Duration::from_secs(45))?;
    if !out.status.success() {
        bail!(
            "ffprobe error: {}",
            out.stderr.lines().last().unwrap_or("unknown")
        );
    }
    let value: Value = serde_json::from_str(&out.stdout).context("invalid ffprobe JSON")?;
    let duration = value
        .get("format")
        .and_then(|f| f.get("duration"))
        .and_then(|d| d.as_str())
        .and_then(|d| d.parse::<f64>().ok())
        .unwrap_or(0.0);
    let mut info = MediaProbe {
        duration,
        ..Default::default()
    };
    parse_streams(&value, &mut info);
    Ok(info)
}

fn has(encoders: &[String], name: &str) -> bool {
    encoders.iter().any(|e| e == name)
}

fn temp_stderr() -> PathBuf {
    portable::temp_dir().join(format!(
        "ff_{}_{}.log",
        std::process::id(),
        FF_COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

fn base_ffmpeg_args(input: &Path) -> Vec<String> {
    vec![
        "-hide_banner".into(),
        "-nostdin".into(),
        "-y".into(),
        "-i".into(),
        input.display().to_string(),
    ]
}

fn push_progress_output(args: &mut Vec<String>, output: &Path) {
    args.extend([
        "-progress".into(),
        "pipe:1".into(),
        "-nostats".into(),
        output.display().to_string(),
    ]);
}

fn spawn_progress_reader(
    stdout: Option<ChildStdout>,
) -> (mpsc::Receiver<String>, std::thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel::<String>();
    let handle = std::thread::spawn(move || {
        if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(|line| line.ok()) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        }
    });
    (rx, handle)
}

fn handle_progress_line(line: &str, duration: f64, progress: &(dyn Fn(f32) + Sync)) {
    let line = line.trim();
    if line == "progress=end" {
        progress(1.0);
        return;
    }
    if let Some(value) = line
        .strip_prefix("out_time_us=")
        .or_else(|| line.strip_prefix("out_time_ms="))
        && let (Ok(us), true) = (value.parse::<f64>(), duration > 0.0)
    {
        progress((us / 1_000_000.0 / duration).clamp(0.0, 0.999) as f32);
    }
}

fn wait_for_ffmpeg(
    child: &mut Child,
    rx: &mpsc::Receiver<String>,
    duration: f64,
    cancel: &AtomicBool,
    progress: &(dyn Fn(f32) + Sync),
) -> Result<Option<ExitStatus>> {
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            bail!("cancelled");
        }
        if let Ok(line) = rx.recv_timeout(Duration::from_millis(120)) {
            handle_progress_line(&line, duration, progress);
        }
        if let Some(done) = child.try_wait()? {
            return Ok(Some(done));
        }
    }
}

fn finish_ffmpeg_status(
    status: Option<ExitStatus>,
    log: &str,
    progress: &(dyn Fn(f32) + Sync),
) -> Result<()> {
    match status {
        Some(status) if status.success() => {
            progress(1.0);
            Ok(())
        }
        Some(status) => {
            let mut tail: Vec<&str> = log
                .lines()
                .filter(|l| !l.trim().is_empty())
                .rev()
                .take(6)
                .collect();
            tail.reverse();
            bail!(
                "ffmpeg error code {:?}: {}",
                status.code(),
                tail.join(" | ")
            )
        }
        None => bail!("could not read ffmpeg status"),
    }
}

/// ffmpeg'i `-progress pipe:1` ile çalıştırır; ilerlemeyi 0..1 olarak bildirir.
fn run_with_progress(
    program: &Path,
    args: &[String],
    duration: f64,
    cancel: &AtomicBool,
    progress: &(dyn Fn(f32) + Sync),
) -> Result<()> {
    let log_path = temp_stderr();
    let log_file = File::create(&log_path)?;
    let mut child = portable::new_command(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(log_file))
        .spawn()
        .with_context(|| format!("could not start ffmpeg: {}", program.display()))?;

    let (rx, reader) = spawn_progress_reader(child.stdout.take());
    let wait_res = wait_for_ffmpeg(&mut child, &rx, duration, cancel, progress);
    let _ = reader.join();
    while rx.try_recv().is_ok() {}

    let log = fs::read_to_string(&log_path).unwrap_or_default();
    let _ = fs::remove_file(&log_path);

    finish_ffmpeg_status(wait_res?, &log, progress)
}

fn pick_hw_encoder(
    gpu: bool,
    encoders: &[String],
    codec_prefix: &str,
) -> Option<(String, &'static str)> {
    if !gpu {
        return None;
    }
    for backend in ["nvenc", "amf", "qsv"] {
        let name = format!("{codec_prefix}_{backend}");
        if has(encoders, &name) {
            let static_backend = match backend {
                "nvenc" => "nvenc",
                "amf" => "amf",
                _ => "qsv",
            };
            return Some((name, static_backend));
        }
    }
    None
}

fn pick_video_encoder(
    settings: &Settings,
    encoders: &[String],
    ext: &str,
) -> (String, &'static str) {
    let speed = settings.encoder_prefer == "speed";
    let mut codec = settings.video_codec.as_str();
    if codec == "auto" {
        codec = if speed { "h264" } else { "h265" };
    }
    // WebM konteyneri yalnızca VP8/VP9/AV1 video + Vorbis/Opus ses destekler.
    // H.264/H.265 seçilmişse webm çıktısında uyumlu bir kodeğe geç.
    if ext == "webm" && matches!(codec, "h264" | "h265" | "hevc" | "auto") {
        if settings.gpu && settings.gpu_av1 && has(encoders, "av1_nvenc")
            || settings.gpu && settings.gpu_av1 && has(encoders, "av1_amf")
            || settings.gpu && settings.gpu_av1 && has(encoders, "av1_qsv")
        {
            codec = "av1";
        } else {
            codec = "vp9";
        }
    }
    let gpu = settings.gpu;
    match codec {
        "h264" => {
            pick_hw_encoder(gpu, encoders, "h264").unwrap_or_else(|| ("libx264".into(), "cpu"))
        }
        "h265" | "hevc" => {
            pick_hw_encoder(gpu, encoders, "hevc").unwrap_or_else(|| ("libx265".into(), "cpu"))
        }
        "av1" => {
            if has(encoders, "libsvtav1") {
                ("libsvtav1".into(), "cpu")
            } else if has(encoders, "libaom-av1") {
                ("libaom-av1".into(), "cpu")
            } else if settings.gpu_av1 {
                pick_hw_encoder(gpu, encoders, "av1").unwrap_or_else(|| ("libx265".into(), "cpu"))
            } else {
                ("libx265".into(), "cpu")
            }
        }
        "vp9" => {
            pick_hw_encoder(gpu, encoders, "vp9").unwrap_or_else(|| ("libvpx-vp9".into(), "cpu"))
        }
        "copy" => ("copy".into(), "copy"),
        _ => pick_hw_encoder(gpu, encoders, "hevc").unwrap_or_else(|| ("libx265".into(), "cpu")),
    }
}

fn audio_codec_for(output: &Path, settings: &Settings) -> (&'static str, bool) {
    let ext = output
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mp4")
        .to_lowercase();
    let preferred = settings.video_audio_codec.as_str();
    if ext == "webm" {
        // WebM yalnızca Vorbis/Opus destekler; AAC/MP3 burada başarısız olur.
        return ("libopus", false);
    }
    match preferred {
        "opus" => ("libopus", false),
        "mp3" => ("libmp3lame", false),
        "aac" => ("aac", false),
        "vorbis" => ("libvorbis", false),
        "flac" => ("flac", true),
        "copy" => ("copy", true),
        _ => ("aac", false),
    }
}

fn build_filters(settings: &Settings, probe: &MediaProbe) -> Option<String> {
    let mut filters = Vec::new();
    if probe.width > 0 && probe.height > 0 {
        let mut target = probe.height;
        if let Some(limit) = settings.video_height_limit() {
            target = target.min(limit);
        }
        if settings.video_scale_pct < 100 {
            target = (target as f64 * settings.video_scale_pct as f64 / 100.0) as u32;
        }
        target = (target.max(2) / 2) * 2;
        if target < probe.height {
            filters.push(format!("scale=-2:{target}:flags=lanczos"));
            filters.push("setsar=1".into());
        }
    }
    if let Some(fps) = settings.video_fps_limit()
        && fps > 0
    {
        filters.push(format!("fps={fps}"));
    }
    if settings.video_denoise {
        filters.push("hqdn3d=1.5:1.5:6:6".into());
    }
    if filters.is_empty() {
        None
    } else {
        Some(filters.join(","))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct VideoUpscale {
    pub target_height: Option<u32>,
    pub factor: Option<u32>,
    pub crf: u8,
    pub preset: &'static str,
}

fn build_upscale_args(
    input: &Path,
    output: &Path,
    scale: &str,
    opts: &VideoUpscale,
) -> Vec<String> {
    let mut args = base_ffmpeg_args(input);
    args.extend([
        "-map".into(),
        "0:v:0".into(),
        "-map".into(),
        "0:a?".into(),
        "-vf".into(),
        scale.into(),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        opts.preset.into(),
        "-crf".into(),
        opts.crf.to_string(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "192k".into(),
        "-movflags".into(),
        "+faststart".into(),
    ]);
    push_progress_output(&mut args, output);
    args
}

pub fn upscale_video(
    input: &Path,
    output: &Path,
    opts: &VideoUpscale,
    ffmpeg: &Path,
    ffprobe: &Path,
    cancel: &AtomicBool,
    progress: &(dyn Fn(f32) + Sync),
) -> Result<MediaProbe> {
    let probe = probe(ffprobe, input)?;
    if !probe.has_video {
        bail!("no video stream found");
    }
    let scale = match (opts.factor, opts.target_height) {
        (Some(factor), _) if factor > 1 => {
            format!("scale=trunc(iw*{factor}/2)*2:trunc(ih*{factor}/2)*2:flags=lanczos")
        }
        (_, Some(height)) => format!("scale=-2:{height}:flags=lanczos"),
        _ => bail!("target resolution is missing"),
    };
    let args = build_upscale_args(input, output, &scale, opts);
    run_with_progress(ffmpeg, &args, probe.duration, cancel, progress)?;
    Ok(probe)
}

fn push_cpu_video_args(args: &mut Vec<String>, encoder: &str, crf: u32) {
    match encoder {
        "libx265" | "libx264" => {
            args.extend([
                "-crf".into(),
                crf.to_string(),
                "-preset".into(),
                "medium".into(),
            ]);
        }
        "libsvtav1" => {
            args.extend([
                "-crf".into(),
                (crf + 3).to_string(),
                "-preset".into(),
                "6".into(),
            ]);
        }
        "libaom-av1" => {
            args.extend([
                "-crf".into(),
                (crf + 3).to_string(),
                "-b:v".into(),
                "0".into(),
                "-cpu-used".into(),
                "6".into(),
            ]);
        }
        "libvpx-vp9" => {
            args.extend([
                "-crf".into(),
                crf.to_string(),
                "-b:v".into(),
                "0".into(),
                "-row-mt".into(),
                "1".into(),
            ]);
        }
        _ => {}
    }
}

fn push_gpu_video_args(args: &mut Vec<String>, backend: &str, settings: &Settings, crf: u32) {
    // Donanım kodlayıcılar kaliteyi CQ/QP ile alır.
    let q = (crf + 2).min(51).to_string();
    let speed = settings.encoder_prefer == "speed";
    match backend {
        "nvenc" => args.extend([
            "-preset".into(),
            if speed { "p1" } else { "p6" }.into(),
            "-tune".into(),
            "hq".into(),
            "-rc".into(),
            "vbr".into(),
            "-cq".into(),
            q,
            "-b:v".into(),
            "0".into(),
            "-gpu".into(),
            settings.gpu_device.to_string(),
        ]),
        "amf" => args.extend([
            "-quality".into(),
            if speed { "speed" } else { "quality" }.into(),
            "-rc".into(),
            "cqp".into(),
            "-qp_i".into(),
            q.clone(),
            "-qp_p".into(),
            q,
        ]),
        _ => args.extend([
            "-preset".into(),
            if speed { "veryfast" } else { "medium" }.into(),
            "-global_quality".into(),
            q,
        ]),
    }
}

fn video_pix_fmt(settings: &Settings, encoder: &str) -> &'static str {
    if settings.video_10bit {
        if matches!(
            encoder,
            "libx265" | "libsvtav1" | "libaom-av1" | "libvpx-vp9"
        ) {
            "yuv420p10le"
        } else if encoder.starts_with("hevc_") || encoder.starts_with("av1_") {
            "p010le"
        } else {
            "yuv420p"
        }
    } else {
        "yuv420p"
    }
}

fn push_video_encode_args(
    args: &mut Vec<String>,
    settings: &Settings,
    probe: &MediaProbe,
    encoder: &str,
    backend: &str,
) {
    let crf = settings.video_crf();
    if backend == "cpu" {
        push_cpu_video_args(args, encoder, crf);
    } else {
        push_gpu_video_args(args, backend, settings, crf);
    }
    args.extend(["-pix_fmt".into(), video_pix_fmt(settings, encoder).into()]);

    if settings.video_tune != "auto"
        && settings.video_tune != "off"
        && (encoder == "libx264" || encoder == "libx265")
    {
        let tune = match settings.video_tune.as_str() {
            "film" => "film",
            "grain" => "grain",
            "animation" => "animation",
            _ => "",
        };
        if !tune.is_empty() {
            args.extend(["-tune".into(), tune.into()]);
        }
    }

    if let Some(filters) = build_filters(settings, probe) {
        args.extend(["-vf".into(), filters]);
    }
}

fn push_video_audio_and_mux(
    args: &mut Vec<String>,
    output: &Path,
    settings: &Settings,
    probe: &MediaProbe,
    ext: &str,
) {
    let (acodec, lossless_audio) = audio_codec_for(output, settings);
    if probe.has_audio {
        args.extend(["-c:a".into(), acodec.to_string()]);
        if acodec != "copy" {
            if !lossless_audio {
                args.extend(["-b:a".into(), format!("{}k", settings.video_audio_kbps())]);
            }
            if settings.audio_mono {
                args.extend(["-ac".into(), "1".into()]);
            }
            if settings.audio_normalize {
                args.extend(["-af".into(), "loudnorm=I=-16:TP=-1.5:LRA=11".into()]);
            }
        }
    } else {
        args.push("-an".into());
    }
    if settings.video_keep_subs && ext == "mkv" {
        args.extend(["-map".into(), "0:s?".into(), "-c:s".into(), "copy".into()]);
    }
    if ext == "mp4" && settings.video_faststart {
        args.extend(["-movflags".into(), "+faststart".into()]);
    }
}

fn build_safe_video_args(
    input: &Path,
    output: &Path,
    ext: &str,
    settings: &Settings,
    probe: &MediaProbe,
) -> (Vec<String>, &'static str) {
    // Güvenli mod: uyumlu bir kodek + 8-bit + basit ölçek.
    let (safe_vcodec, safe_acodec) = if ext == "webm" {
        ("libvpx-vp9", "libopus")
    } else {
        ("libx264", "aac")
    };
    let crf = settings.video_crf();
    let mut fallback = base_ffmpeg_args(input);
    fallback.extend([
        "-map".into(),
        "0:v:0".into(),
        "-map".into(),
        "0:a?".into(),
        "-c:v".into(),
        safe_vcodec.into(),
        "-crf".into(),
        crf.to_string(),
    ]);
    if safe_vcodec == "libx264" {
        fallback.extend(["-preset".into(), "medium".into()]);
    } else {
        fallback.extend(["-b:v".into(), "0".into()]);
    }
    fallback.extend(["-pix_fmt".into(), "yuv420p".into()]);
    if let Some(filters) = build_filters(settings, probe) {
        fallback.extend(["-vf".into(), filters]);
    }
    if probe.has_audio {
        let bitrate = if safe_acodec == "libopus" {
            "128k"
        } else {
            "160k"
        };
        fallback.extend([
            "-c:a".into(),
            safe_acodec.into(),
            "-b:a".into(),
            bitrate.into(),
        ]);
    } else {
        fallback.push("-an".into());
    }
    if ext == "mp4" {
        fallback.extend(["-movflags".into(), "+faststart".into()]);
    }
    push_progress_output(&mut fallback, output);
    (fallback, safe_vcodec)
}

fn compress_video_safe(
    input: &Path,
    output: &Path,
    ext: &str,
    settings: &Settings,
    probe: &MediaProbe,
    env: &FfmpegEnv<'_>,
    progress: &(dyn Fn(f32) + Sync),
) -> Result<String> {
    let (fallback, safe_vcodec) = build_safe_video_args(input, output, ext, settings, probe);
    run_with_progress(env.ffmpeg, &fallback, probe.duration, env.cancel, progress)
        .context("encoding failed even in safe mode")?;
    Ok(format!("{safe_vcodec} • safe mode"))
}

fn format_video_note(encoder: &str, backend: &str, crf: u32) -> String {
    format!(
        "{} • {} • {}",
        encoder,
        if backend == "cpu" { "CPU" } else { backend },
        if backend == "cpu" {
            format!("CRF {crf}")
        } else {
            format!("CQ {}", (crf + 2).min(51))
        }
    )
}

pub fn compress_video(
    input: &Path,
    output: &Path,
    settings: &Settings,
    env: &FfmpegEnv<'_>,
    progress: &(dyn Fn(f32) + Sync),
) -> Result<String> {
    let probe = probe(env.ffprobe, input).unwrap_or_default();
    let ext = output
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mp4")
        .to_lowercase();
    let (encoder, backend) = pick_video_encoder(settings, env.encoders, &ext);
    let mut args = base_ffmpeg_args(input);
    args.extend([
        "-map".into(),
        "0:v:0".into(),
        "-map".into(),
        "0:a?".into(),
        "-map_metadata".into(),
        if settings.keep_metadata { "0" } else { "-1" }.into(),
        "-c:v".into(),
        encoder.clone(),
    ]);
    if encoder != "copy" {
        push_video_encode_args(&mut args, settings, &probe, &encoder, backend);
        push_video_audio_and_mux(&mut args, output, settings, &probe, &ext);
    } else {
        args.extend(["-c:a".into(), "copy".into()]);
    }
    push_progress_output(&mut args, output);

    if run_with_progress(env.ffmpeg, &args, probe.duration, env.cancel, progress).is_ok() {
        return Ok(format_video_note(&encoder, backend, settings.video_crf()));
    }
    if env.cancel.load(Ordering::Relaxed) {
        bail!("cancelled");
    }
    compress_video_safe(input, output, &ext, settings, &probe, env, progress)
}

fn audio_encoder_for(output: &Path, _settings: &Settings) -> (&'static str, bool) {
    let ext = output
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("m4a")
        .to_lowercase();
    match ext.as_str() {
        "mp3" => ("libmp3lame", false),
        "m4a" | "aac" => ("aac", false),
        "opus" => ("libopus", false),
        "ogg" => ("libvorbis", false),
        "flac" => ("flac", true),
        "wma" => ("wmav2", false),
        _ => ("libmp3lame", false),
    }
}

fn build_audio_args(
    input: &Path,
    output: &Path,
    settings: &Settings,
    encoder: &str,
    lossless: bool,
) -> Vec<String> {
    let mut args = base_ffmpeg_args(input);
    args.extend([
        "-map".into(),
        "0:a:0".into(),
        "-vn".into(),
        "-c:a".into(),
        encoder.into(),
    ]);
    let bitrate = settings.music_kbps();
    if !lossless {
        let kbps = if encoder == "libopus" {
            (bitrate as f32 * 0.65) as u32
        } else {
            bitrate
        };
        args.extend(["-b:a".into(), format!("{kbps}k")]);
    }
    if settings.audio_mono {
        args.extend(["-ac".into(), "1".into()]);
    }
    if settings.audio_normalize {
        args.extend(["-af".into(), "loudnorm=I=-16:TP=-1.5:LRA=11".into()]);
    }
    args.extend([
        "-map_metadata".into(),
        if settings.keep_metadata { "0" } else { "-1" }.into(),
    ]);
    push_progress_output(&mut args, output);
    args
}

pub fn compress_audio(
    input: &Path,
    output: &Path,
    settings: &Settings,
    env: &FfmpegEnv<'_>,
    progress: &(dyn Fn(f32) + Sync),
) -> Result<String> {
    let probe = probe(env.ffprobe, input).unwrap_or_default();
    let (encoder, lossless) = audio_encoder_for(output, settings);
    let args = build_audio_args(input, output, settings, encoder, lossless);
    run_with_progress(env.ffmpeg, &args, probe.duration, env.cancel, progress)?;
    let bitrate = settings.music_kbps();
    Ok(format!(
        "{encoder} • {}",
        if lossless {
            "lossless".to_string()
        } else {
            format!("{bitrate}k")
        }
    ))
}

struct GifSpec<'a> {
    scale: &'a str,
    fps: u32,
    width: u32,
    duration: f64,
}

fn compress_gif_webp(
    input: &Path,
    output: &Path,
    spec: &GifSpec<'_>,
    env: &FfmpegEnv<'_>,
    progress: &(dyn Fn(f32) + Sync),
) -> Result<String> {
    if !has(env.encoders, "libwebp_anim") {
        bail!("libwebp_anim encoder is not available");
    }
    let mut args = base_ffmpeg_args(input);
    args.extend([
        "-vf".into(),
        spec.scale.into(),
        "-c:v".into(),
        "libwebp_anim".into(),
        "-lossless".into(),
        "0".into(),
        "-q:v".into(),
        "60".into(),
        "-loop".into(),
        "0".into(),
    ]);
    push_progress_output(&mut args, output);
    run_with_progress(env.ffmpeg, &args, spec.duration, env.cancel, progress)?;
    Ok(format!(
        "Animated WebP • {} fps / {}px",
        spec.fps, spec.width
    ))
}

fn compress_gif_palette(
    input: &Path,
    output: &Path,
    settings: &Settings,
    spec: &GifSpec<'_>,
    env: &FfmpegEnv<'_>,
    progress: &(dyn Fn(f32) + Sync),
) -> Result<String> {
    let palette = portable::temp_dir().join(format!(
        "palette_{}_{}.png",
        std::process::id(),
        FF_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let mut pass1 = base_ffmpeg_args(input);
    pass1.extend([
        "-vf".into(),
        format!("{},palettegen=stats_mode=diff", spec.scale),
        "-frames:v".into(),
        "1".into(),
        palette.display().to_string(),
    ]);
    if let Err(err) = portable::run_capture(env.ffmpeg, &pass1, Duration::from_secs(300)) {
        let _ = fs::remove_file(&palette);
        return Err(err);
    }
    let filter = format!(
        "{}[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle",
        spec.scale
    );
    let mut args = base_ffmpeg_args(input);
    args.extend([
        "-i".into(),
        palette.display().to_string(),
        "-lavfi".into(),
        filter,
        "-loop".into(),
        if settings.gif_loop { "0" } else { "-1" }.into(),
    ]);
    push_progress_output(&mut args, output);
    let result = run_with_progress(env.ffmpeg, &args, spec.duration, env.cancel, progress);
    let _ = fs::remove_file(&palette);
    result?;
    Ok(format!(
        "GIF • {} fps / {}px • 2-pass palette",
        spec.fps, spec.width
    ))
}

fn compress_gif_mp4(
    input: &Path,
    output: &Path,
    settings: &Settings,
    spec: &GifSpec<'_>,
    env: &FfmpegEnv<'_>,
    progress: &(dyn Fn(f32) + Sync),
) -> Result<String> {
    let mut args = base_ffmpeg_args(input);
    args.extend([
        "-vf".into(),
        spec.scale.into(),
        "-c:v".into(),
        "libx264".into(),
        "-crf".into(),
        (settings.video_crf() + 1).to_string(),
        "-preset".into(),
        "medium".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-movflags".into(),
        "+faststart".into(),
        "-an".into(),
    ]);
    push_progress_output(&mut args, output);
    run_with_progress(env.ffmpeg, &args, spec.duration, env.cancel, progress)?;
    Ok(format!("MP4 • {} fps / {}px", spec.fps, spec.width))
}

pub fn compress_gif(
    input: &Path,
    output: &Path,
    settings: &Settings,
    env: &FfmpegEnv<'_>,
    progress: &(dyn Fn(f32) + Sync),
) -> Result<String> {
    let probe = probe(env.ffprobe, input).unwrap_or_default();
    let ext = output
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mp4")
        .to_lowercase();
    let fps = match settings.gif_max_fps.as_str() {
        "auto" | "" => settings.gif_fps(),
        value => value.parse::<u32>().unwrap_or(settings.gif_fps()),
    };
    let width = settings.gif_width();
    let scale = format!("fps={fps},scale={width}:-2:flags=lanczos");
    let spec = GifSpec {
        scale: &scale,
        fps,
        width,
        duration: probe.duration,
    };

    if ext == "webp" {
        return compress_gif_webp(input, output, &spec, env, progress);
    }
    if ext == "gif" {
        return compress_gif_palette(input, output, settings, &spec, env, progress);
    }
    compress_gif_mp4(input, output, settings, &spec, env, progress)
}

pub fn verify_output(ffprobe: &Path, output: &Path) -> bool {
    probe(ffprobe, output)
        .map(|p| p.duration > 0.0 || p.has_video || p.has_audio)
        .unwrap_or(false)
}

pub fn media_duration(ffprobe: &Path, input: &Path) -> Option<f64> {
    probe(ffprobe, input).ok().map(|p| p.duration)
}

pub fn encode_image_avif(
    input: &Path,
    output: &Path,
    settings: &Settings,
    ffmpeg: &Path,
) -> Result<()> {
    let crf = (20.0 + 40.0 * settings.t()).round() as u32;
    let args = vec![
        "-hide_banner".into(),
        "-nostdin".into(),
        "-y".into(),
        "-i".into(),
        input.display().to_string(),
        "-frames:v".into(),
        "1".into(),
        "-c:v".into(),
        "libsvtav1".into(),
        "-crf".into(),
        crf.to_string(),
        "-preset".into(),
        "6".into(),
        output.display().to_string(),
    ];
    let result = portable::run_capture(ffmpeg, &args, Duration::from_secs(300))?;
    if result.status.success() && output.is_file() {
        Ok(())
    } else {
        Err(anyhow!(
            "could not encode AVIF: {}",
            result.stderr.lines().last().unwrap_or("unknown")
        ))
    }
}
