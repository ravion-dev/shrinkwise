use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, anyhow, bail};
use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageDecoder, ImageEncoder, RgbImage};

use crate::portable;
use crate::settings::Settings;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

fn temp_path(tag: &str, ext: &str) -> PathBuf {
    let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    portable::temp_dir().join(format!("img_{}_{}_{}.{}", std::process::id(), n, tag, ext))
}

fn ffmpeg_convert_to_png(input: &Path, ffmpeg: &Path) -> Result<PathBuf> {
    let out = temp_path("decode", "png");
    let args = vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-nostdin".to_string(),
        "-y".to_string(),
        "-i".to_string(),
        input.display().to_string(),
        "-frames:v".to_string(),
        "1".to_string(),
        out.display().to_string(),
    ];
    let result = portable::run_capture(ffmpeg, &args, std::time::Duration::from_secs(120))?;
    if !result.status.success() || !out.is_file() {
        let _ = std::fs::remove_file(&out);
        bail!(
            "could not open image with ffmpeg: {}",
            result.stderr.lines().last().unwrap_or("unknown error")
        );
    }
    Ok(out)
}

fn decode_with_orientation(
    reader: image::ImageReader<std::io::BufReader<File>>,
) -> image::ImageResult<DynamicImage> {
    let mut decoder = reader.into_decoder()?;
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);
    Ok(img)
}

pub fn decode(input: &Path, ffmpeg: Option<&Path>) -> Result<(DynamicImage, Option<PathBuf>)> {
    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let needs_ffmpeg = matches!(
        ext.as_str(),
        "avif"
            | "heic"
            | "heif"
            | "jxl"
            | "jp2"
            | "j2k"
            | "hdr"
            | "exr"
            | "raw"
            | "cr2"
            | "cr3"
            | "nef"
            | "arw"
            | "dng"
            | "orf"
            | "rw2"
            | "raf"
            | "sr2"
            | "pef"
            | "srw"
    );

    if !needs_ffmpeg
        && let Ok(reader) = image::ImageReader::open(input)
        && let Ok(reader) = reader.with_guessed_format()
        && let Ok(img) = decode_with_orientation(reader)
    {
        return Ok((img, None));
    }

    if let Some(ffmpeg) = ffmpeg {
        let temp = ffmpeg_convert_to_png(input, ffmpeg)?;
        let reader = image::ImageReader::open(&temp)
            .with_context(|| format!("could not open temporary image: {}", temp.display()))?
            .with_guessed_format()?;
        let img = decode_with_orientation(reader).context("could not decode ffmpeg output")?;
        return Ok((img, Some(temp)));
    }
    bail!("FFmpeg is required for this image format")
}

fn flatten_white(img: &DynamicImage) -> RgbImage {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut rgb = RgbImage::new(w, h);
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let [r, g, b, a] = pixel.0;
        let alpha = a as u16;
        let inv = 255 - alpha;
        let blend = |c: u8| -> u8 { (((c as u16 * alpha) + (255 * inv)) / 255).min(255) as u8 };
        rgb.put_pixel(x, y, image::Rgb([blend(r), blend(g), blend(b)]));
    }
    rgb
}

fn resize_if_needed(img: DynamicImage, settings: &Settings) -> DynamicImage {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return img;
    }
    let mut scale = 1.0f32;

    if settings.image_max_side_pct < 100 {
        scale = scale.min(settings.image_max_side_pct as f32 / 100.0);
    }
    if settings.image_max_mp > 0.0 {
        let max_pixels = settings.image_max_mp as f64 * 1_000_000.0;
        let pixels = w as f64 * h as f64;
        if pixels > max_pixels {
            scale = scale.min((max_pixels / pixels).sqrt() as f32);
        }
    }
    // "Maksimum" seviyesinde çok büyük görselleri makul bir üst sınıra çek.
    if settings.lossy >= 80 && settings.smart_mode {
        scale = scale.min(0.92);
    }

    if scale >= 0.999 {
        return img;
    }
    let nw = ((w as f32 * scale).round() as u32).max(1);
    let nh = ((h as f32 * scale).round() as u32).max(1);
    img.resize_exact(nw, nh, FilterType::Lanczos3)
}

fn encode_jpeg(img: &DynamicImage, output: &Path, quality: u8) -> Result<()> {
    let rgb = if img.color().has_alpha() {
        DynamicImage::ImageRgb8(flatten_white(img))
    } else {
        DynamicImage::ImageRgb8(img.to_rgb8())
    };
    let rgb = rgb.to_rgb8();
    let (w, h) = rgb.dimensions();
    let file = File::create(output)?;
    let mut writer = BufWriter::new(file);
    let encoder = jpeg_encoder::Encoder::new(&mut writer, quality.clamp(1, 100));
    encoder
        .encode(
            rgb.as_raw(),
            w as u16,
            h as u16,
            jpeg_encoder::ColorType::Rgb,
        )
        .map_err(|e| anyhow!("JPEG encoding error: {e}"))?;
    writer.flush()?;
    Ok(())
}

fn encode_png(img: &DynamicImage, output: &Path) -> Result<()> {
    let file = File::create(output)?;
    let writer = BufWriter::new(file);
    let encoder = PngEncoder::new_with_quality(writer, CompressionType::Best, PngFilter::Adaptive);
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    encoder
        .write_image(rgba.as_raw(), w, h, image::ExtendedColorType::Rgba8)
        .context("PNG encoding error")?;
    Ok(())
}

fn encode_webp(img: &DynamicImage, output: &Path, quality: u8, lossless: bool) -> Result<()> {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let encoder = webp::Encoder::from_rgba(rgba.as_raw(), w, h);
    let memory = if lossless {
        encoder.encode_lossless()
    } else {
        encoder.encode(quality.clamp(1, 100) as f32)
    };
    std::fs::write(output, &*memory).context("WebP write error")?;
    Ok(())
}

fn encode_avif(input: &Path, output: &Path, settings: &Settings, ffmpeg: &Path) -> Result<()> {
    // CRF: kayıplılık arttıkça yükselir.
    let crf = (20.0 + 40.0 * settings.t()).round() as u32;
    let base = vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-nostdin".to_string(),
        "-y".to_string(),
        "-i".to_string(),
        input.display().to_string(),
        "-frames:v".to_string(),
        "1".to_string(),
        "-crf".to_string(),
        crf.to_string(),
    ];
    let mut tries: Vec<Vec<String>> = Vec::new();
    let mut svt = base.clone();
    svt.extend([
        "-c:v".to_string(),
        "libsvtav1".to_string(),
        "-preset".to_string(),
        "6".to_string(),
    ]);
    svt.push(output.display().to_string());
    tries.push(svt);
    let mut aom = base;
    aom.extend([
        "-c:v".to_string(),
        "libaom-av1".to_string(),
        "-still-picture".to_string(),
        "1".to_string(),
        "-cpu-used".to_string(),
        "6".to_string(),
    ]);
    aom.push(output.display().to_string());
    tries.push(aom);

    let mut last_error = String::new();
    for args in tries {
        let result = portable::run_capture(ffmpeg, &args, std::time::Duration::from_secs(300))?;
        if result.status.success() && output.is_file() {
            return Ok(());
        }
        last_error = result.stderr.lines().last().unwrap_or("").to_string();
    }
    bail!("could not encode AVIF: {last_error}")
}

/// Kaynak görseli hedef uzantıya göre sıkıştırır.
/// Dönen `String` kullanıcıya gösterilecek yöntem notudur.
pub fn compress(
    input: &Path,
    output: &Path,
    settings: &Settings,
    ffmpeg: Option<&Path>,
) -> Result<String> {
    let target_ext = output
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jpg")
        .to_lowercase();

    if target_ext == "avif" {
        let ffmpeg = ffmpeg.ok_or_else(|| anyhow!("FFmpeg is required for AVIF"))?;
        encode_avif(input, output, settings, ffmpeg)?;
        return Ok(format!(
            "AVIF • CRF {}",
            (20.0 + 40.0 * settings.t()).round() as u32
        ));
    }

    let (img, temp) = decode(input, ffmpeg)?;
    let img = resize_if_needed(img, settings);

    let note = match target_ext.as_str() {
        "jpg" | "jpeg" | "jpe" | "jfif" => {
            let quality = settings.image_quality();
            encode_jpeg(&img, output, quality)?;
            format!("JPEG • quality {quality}")
        }
        "png" => {
            encode_png(&img, output)?;
            "PNG • lossless re-encode".to_string()
        }
        "webp" => {
            let quality = settings.image_quality();
            encode_webp(&img, output, quality, settings.lossless_mode)?;
            if settings.lossless_mode {
                "WebP • lossless".to_string()
            } else {
                format!("WebP • quality {quality}")
            }
        }
        "bmp" => {
            encode_png(&img, output)?;
            "PNG • lossless".to_string()
        }
        other => {
            let quality = settings.image_quality();
            encode_jpeg(&img, output, quality)?;
            format!("JPEG • {other} -> jpg")
        }
    };

    if let Some(temp) = temp {
        let _ = std::fs::remove_file(temp);
    }
    Ok(note)
}

pub fn verify(path: &Path, ffprobe: Option<&Path>) -> bool {
    // 1) image crate ile decode edilebiliyorsa (JPEG/PNG/WebP/GIF/...) geçerli say.
    let image_ok = image::ImageReader::open(path)
        .ok()
        .and_then(|r| r.with_guessed_format().ok())
        .and_then(|r| r.into_dimensions().ok())
        .map(|(w, h)| w > 0 && h > 0)
        .unwrap_or(false);
    if image_ok {
        return true;
    }
    // 2) image crate desteklemiyorsa (AVIF vb.) ffprobe ile kontrol et.
    if let Some(fp) = ffprobe
        && crate::ffmpeg_engine::verify_output(fp, path)
    {
        return true;
    }
    // 3) Son çare: dosya boş değilse ve geçerli bir görüntü "ftyp" kutusu taşıyorsa (AVIF/HEIC) geçerli say.
    if let Ok(bytes) = std::fs::read(path) {
        if bytes.len() > 32 && &bytes[4..8] == b"ftyp" {
            return true;
        }
        // JPEG/PNG/WebP sihirli başlıkları (image crate okuyamadıysa bile)
        if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return true;
        }
        if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
            return true;
        }
        if bytes.starts_with(b"RIFF") && bytes.len() >= 12 && &bytes[8..12] == b"WEBP" {
            return true;
        }
    }
    false
}
