use std::fs::File;
use std::io::{self, Cursor, Read, Seek, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const MAX_NESTED: u64 = 256 * 1024 * 1024;

const STORE_EXTS: &[&str] = &[
    "jpg", "jpeg", "jpe", "png", "gif", "webp", "avif", "heic", "heif", "jxl", "mp3", "aac", "m4a",
    "ogg", "opus", "mp4", "m4v", "mkv", "webm", "mov", "avi", "zip", "rar", "7z", "gz", "xz",
    "zst", "br", "woff", "woff2",
];

fn lower_ext(name: &str) -> String {
    Path::new(name)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase()
}

fn is_zip_name(name: &str) -> bool {
    matches!(
        lower_ext(name).as_str(),
        "zip"
            | "docx"
            | "xlsx"
            | "pptx"
            | "docm"
            | "xlsm"
            | "pptm"
            | "odt"
            | "ods"
            | "odp"
            | "jar"
            | "whl"
            | "egg"
            | "apk"
            | "epub"
            | "xpi"
            | "cbz"
    )
}

fn options_for<R: Read>(entry: &zip::read::ZipFile<'_, R>) -> SimpleFileOptions {
    let stored = STORE_EXTS.contains(&lower_ext(entry.name()).as_str());
    let method = if stored {
        CompressionMethod::Stored
    } else {
        CompressionMethod::Deflated
    };
    let mut options = SimpleFileOptions::default()
        .compression_method(method)
        .large_file(entry.size() > u32::MAX as u64);
    if !stored {
        options = options.compression_level(Some(9));
    }
    if let Some(mode) = entry.unix_mode() {
        options = options.unix_permissions(mode);
    }
    options
}

fn copy_entries<R: Read + Seek, W: Write + Seek>(
    reader: R,
    writer: W,
    depth: u32,
) -> Result<(W, u64)> {
    let mut archive = ZipArchive::new(reader).context("not a readable ZIP container")?;
    let mut zip = ZipWriter::new(writer);
    let mut entries = 0u64;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .with_context(|| format!("could not read ZIP entry #{index}"))?;
        let name = entry.name().to_string();
        if entry.is_dir() || name.ends_with('/') {
            zip.add_directory(name, SimpleFileOptions::default())?;
            entries += 1;
            continue;
        }
        let options = options_for(&entry);
        zip.start_file(name.clone(), options)?;
        if depth > 0 && is_zip_name(&name) && entry.size() <= MAX_NESTED {
            let mut bytes = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut bytes)?;
            let rebuilt = rebuild_bytes(&bytes, depth - 1).ok();
            match rebuilt {
                Some(rebuilt) if rebuilt.len() < bytes.len() => zip.write_all(&rebuilt)?,
                _ => zip.write_all(&bytes)?,
            }
        } else {
            io::copy(&mut entry, &mut zip)?;
        }
        entries += 1;
    }
    let writer = zip.finish()?;
    Ok((writer, entries))
}

fn rebuild_bytes(bytes: &[u8], depth: u32) -> Result<Vec<u8>> {
    let cursor = Cursor::new(Vec::new());
    let (cursor, _) = copy_entries(Cursor::new(bytes), cursor, depth)?;
    Ok(cursor.into_inner())
}

fn verify(path: &Path, expected: u64) -> Result<()> {
    let file = File::open(path)?;
    let archive = ZipArchive::new(file).context("output is not a valid ZIP container")?;
    if archive.len() as u64 != expected {
        bail!(
            "output ZIP has {} entries, expected {expected}",
            archive.len()
        );
    }
    Ok(())
}

pub fn compress_zip(input: &Path, output: &Path, depth: u32) -> Result<String> {
    let source =
        File::open(input).with_context(|| format!("could not open: {}", input.display()))?;
    let target = File::create(output)?;
    let (target, entries) = copy_entries(source, target, depth)?;
    target.sync_all().ok();
    drop(target);
    verify(output, entries)?;
    let nested = if depth > 0 { " · nested" } else { "" };
    Ok(format!("ZIP level 9 · {entries} entries{nested}"))
}
