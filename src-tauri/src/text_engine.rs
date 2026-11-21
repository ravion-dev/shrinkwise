use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, bail};
use flate2::Compression;
use flate2::write::GzEncoder;

static TEXT_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn compress(
    input: &Path,
    temp_dir: &Path,
    lossy: u8,
    max_bytes: u64,
) -> Result<(PathBuf, String, String)> {
    let size = fs::metadata(input)?.len();
    if size > max_bytes {
        bail!("text file is too large ({size} bytes)");
    }
    let base = temp_dir.join(format!(
        ".swtext-{}-{}",
        std::process::id(),
        TEXT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let mut candidates: Vec<(PathBuf, &str)> = Vec::new();

    let br = base.with_extension("br");
    {
        let file = BufWriter::new(File::create(&br)?);
        let quality = if lossy >= 70 { 11 } else { 9 };
        let mut writer = brotli::CompressorWriter::new(file, 64 * 1024, quality, 22);
        copy_into(input, &mut writer)?;
        writer.flush()?;
    }
    candidates.push((br, "Brotli"));

    let xz = base.with_extension("xz");
    {
        let file = BufWriter::new(File::create(&xz)?);
        let preset = if lossy >= 70 { 9 } else { 6 };
        let mut writer = xz2::write::XzEncoder::new(file, preset);
        copy_into(input, &mut writer)?;
        writer.finish()?;
    }
    candidates.push((xz, "XZ"));

    let zst = base.with_extension("zst");
    {
        let file = BufWriter::new(File::create(&zst)?);
        let level = if lossy >= 70 { 19 } else { 12 };
        let mut writer = zstd::stream::write::Encoder::new(file, level)?;
        copy_into(input, &mut writer)?;
        writer.finish()?;
    }
    candidates.push((zst, "Zstandard"));

    let gz = base.with_extension("gz");
    {
        let file = BufWriter::new(File::create(&gz)?);
        let mut writer = GzEncoder::new(file, Compression::best());
        copy_into(input, &mut writer)?;
        writer.finish()?;
    }
    candidates.push((gz, "Gzip"));

    let mut best: Option<(PathBuf, &str, u64)> = None;
    for (path, name) in candidates {
        let len = fs::metadata(&path).map(|m| m.len()).unwrap_or(u64::MAX);
        if best
            .as_ref()
            .map(|(_, _, best_len)| len < *best_len)
            .unwrap_or(true)
        {
            if let Some((old, _, _)) = best.take() {
                let _ = fs::remove_file(old);
            }
            best = Some((path, name, len));
        } else {
            let _ = fs::remove_file(&path);
        }
    }

    let Some((best_path, name, len)) = best else {
        bail!("lossless compression failed");
    };
    if len >= size {
        let _ = fs::remove_file(&best_path);
        bail!("no gain");
    }
    let ext = best_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin")
        .to_string();
    let ratio = 100.0 - (len as f64 / size.max(1) as f64 * 100.0);
    Ok((best_path, ext, format!("{name} • {ratio:.0}% saved")))
}

fn copy_into(input: &Path, writer: &mut impl Write) -> Result<()> {
    let mut file = File::open(input).context("could not open source file")?;
    std::io::copy(&mut file, writer)?;
    Ok(())
}

fn raw_block_end(lower: &str, start: usize) -> Option<usize> {
    for tag in ["script", "style", "pre"] {
        let prefix = format!("<{tag}");
        if lower[start..].starts_with(&prefix) {
            let boundary = lower.as_bytes().get(start + prefix.len()).copied();
            if !boundary.is_some_and(|b| b == b'>' || b == b'/' || b.is_ascii_whitespace()) {
                continue;
            }
            let close = format!("</{tag}");
            let pos = lower[start..].find(&close)?;
            let after = lower[start + pos..].find('>')?;
            return Some(start + pos + after + 1);
        }
    }
    None
}

/// Removes XML comments and insignificant whitespace. Script, style and pre
/// blocks are copied verbatim; the SVG element structure is left untouched.
pub fn minify_svg_text(src: &str) -> String {
    let lower = src.to_ascii_lowercase();
    let mut out = String::with_capacity(src.len());
    let mut i = 0usize;
    while i < src.len() {
        if lower[i..].starts_with("<!--")
            && let Some(pos) = lower[i + 4..].find("-->")
        {
            i += 4 + pos + 3;
            continue;
        }
        if let Some(end) = raw_block_end(&lower, i) {
            out.push_str(&src[i..end]);
            i = end;
            continue;
        }
        let Some(ch) = src[i..].chars().next() else {
            break;
        };
        if ch.is_whitespace() {
            i += ch.len_utf8();
            while i < src.len() {
                let Some(next) = src[i..].chars().next() else {
                    break;
                };
                if !next.is_whitespace() {
                    break;
                }
                i += next.len_utf8();
            }
            let before_tag = out.ends_with('>');
            let next_is_tag = src[i..].starts_with('<');
            if !(before_tag && next_is_tag) {
                out.push(' ');
            }
            continue;
        }
        out.push(ch);
        i += ch.len_utf8();
    }
    out.trim().to_string()
}

/// Minifies an SVG without changing its extension or semantics.
pub fn minify_svg(input: &Path, output: &Path) -> Result<String> {
    let size = fs::metadata(input)?.len();
    let text = fs::read_to_string(input).context("SVG is not valid UTF-8 text")?;
    let minified = minify_svg_text(&text);
    if minified.len() as u64 >= size {
        bail!("no gain");
    }
    if !minified.contains("<svg") {
        bail!("SVG root element is missing");
    }
    fs::write(output, minified.as_bytes())?;
    let ratio = 100.0 - (minified.len() as f64 / size.max(1) as f64 * 100.0);
    Ok(format!("SVG minify • {ratio:.0}% saved"))
}

#[cfg(test)]
mod tests {
    use super::minify_svg_text;

    #[test]
    fn removes_comments() {
        let input = "<svg><!-- header -->  <rect width=\"10\" height=\"10\"/><!-- end --></svg>";
        assert_eq!(
            minify_svg_text(input),
            "<svg><rect width=\"10\" height=\"10\"/></svg>"
        );
    }

    #[test]
    fn preserves_preserve_aspect_ratio_and_minifies_inside() {
        let input = "<svg><preserveAspectRatio> <!-- note -->  xMidYMid  </preserveAspectRatio><pre>  keep   spaces  </pre></svg>";
        assert_eq!(
            minify_svg_text(input),
            "<svg><preserveAspectRatio> xMidYMid </preserveAspectRatio><pre>  keep   spaces  </pre></svg>"
        );
    }

    #[test]
    fn preserves_style_block_verbatim() {
        let input = "<svg><style>  .cls { fill: red; }  </style>  <rect   class=\"cls\"  /></svg>";
        assert_eq!(
            minify_svg_text(input),
            "<svg><style>  .cls { fill: red; }  </style><rect class=\"cls\" /></svg>"
        );
    }
}
