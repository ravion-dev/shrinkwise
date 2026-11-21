use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::{Result, bail};

use crate::portable;
use crate::settings::Settings;

fn pdf_settings(mode: &str) -> &'static str {
    match mode {
        "max" => "/screen",
        "quality" | "text" => "/prepress",
        _ => "/ebook",
    }
}

fn looks_like_pdf(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if meta.len() < 512 {
        return false;
    }
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    bytes.starts_with(b"%PDF-")
}

/// Recompresses a PDF with Ghostscript, falling back to qpdf. Both keep the
/// PDF structure intact; image resolution is what actually shrinks the file.
pub fn compress(
    input: &Path,
    output: &Path,
    settings: &Settings,
    ghostscript: Option<&Path>,
    qpdf: Option<&Path>,
) -> Result<String> {
    let mut last_error: Option<anyhow::Error> = None;

    if let Some(gs) = ghostscript {
        let _ = fs::remove_file(output);
        let dpi = if settings.pdf_raster_dpi > 0 {
            settings.pdf_raster_dpi.clamp(48, 600)
        } else {
            let base = (((200.0 - 128.0 * settings.t()) / 2.0).round() as u32 * 2).clamp(60, 300);
            match settings.pdf_mode.as_str() {
                "max" => base.min(120),
                "quality" | "text" => base.max(150),
                _ => base,
            }
        };
        let args = vec![
            "-sDEVICE=pdfwrite".to_string(),
            "-dCompatibilityLevel=1.7".to_string(),
            "-dNOPAUSE".to_string(),
            "-dBATCH".to_string(),
            "-dQUIET".to_string(),
            format!("-dPDFSETTINGS={}", pdf_settings(&settings.pdf_mode)),
            "-dDetectDuplicateImages=true".to_string(),
            "-dCompressFonts=true".to_string(),
            "-dSubsetFonts=true".to_string(),
            format!("-dColorImageResolution={dpi}"),
            format!("-dGrayImageResolution={dpi}"),
            format!("-dMonoImageResolution={dpi}"),
            format!("-sOutputFile={}", output.display()),
            input.display().to_string(),
        ];
        match portable::run_capture(gs, &args, Duration::from_secs(1800)) {
            Ok(captured) if captured.status.success() && looks_like_pdf(output) => {
                return Ok(format!(
                    "Ghostscript {dpi} dpi · {}",
                    pdf_settings(&settings.pdf_mode)
                ));
            }
            Ok(captured) => {
                let text = if captured.stderr.trim().is_empty() {
                    captured.stdout
                } else {
                    captured.stderr
                };
                last_error = Some(anyhow::anyhow!("Ghostscript failed: {}", first_line(&text)));
            }
            Err(err) => last_error = Some(err),
        }
    }

    if let Some(qpdf) = qpdf {
        let _ = fs::remove_file(output);
        let args = vec![
            "--object-streams=generate".to_string(),
            "--recompress-flate".to_string(),
            "--compression-level=9".to_string(),
            input.display().to_string(),
            output.display().to_string(),
        ];
        match portable::run_capture(qpdf, &args, Duration::from_secs(1800)) {
            Ok(captured) if captured.status.success() && looks_like_pdf(output) => {
                return Ok("qpdf recompress".into());
            }
            Ok(captured) => {
                let text = if captured.stderr.trim().is_empty() {
                    captured.stdout
                } else {
                    captured.stderr
                };
                last_error = Some(anyhow::anyhow!("qpdf failed: {}", first_line(&text)));
            }
            Err(err) => last_error = Some(err),
        }
    }

    match last_error {
        Some(err) => Err(err.context("PDF compression failed")),
        None => bail!("PDF engine not available; install Ghostscript or qpdf in Settings > Tools"),
    }
}

fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("unknown error")
        .to_string()
}
