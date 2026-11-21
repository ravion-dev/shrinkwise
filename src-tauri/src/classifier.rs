use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::Result;
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Serialize;
use walkdir::{DirEntry, WalkDir};

use crate::settings::Settings;

pub const IMAGE_EXTS: &[&str] = &[
    "jpg", "jpeg", "jpe", "jfif", "png", "bmp", "tif", "tiff", "webp", "ppm", "pgm", "pbm", "pnm",
    "pcx", "tga", "dds", "ico", "cur", "qoi", "jxl",
];

pub const FFMPEG_IMAGE_EXTS: &[&str] = &[
    "avif", "heic", "heif", "hdr", "exr", "jp2", "j2k", "dpx", "raw", "cr2", "cr3", "nef", "arw",
    "dng", "orf", "rw2", "raf", "sr2", "pef", "srw",
];

pub const VIDEO_EXTS: &[&str] = &[
    "mp4", "m4v", "mov", "mkv", "flv", "avi", "webm", "wmv", "mpg", "mpeg", "3gp", "ts", "mts",
    "m2ts", "ogv", "vob", "asf", "f4v", "rm", "rmvb", "divx",
];

pub const AUDIO_EXTS: &[&str] = &[
    "mp3", "wav", "flac", "aiff", "aif", "aac", "m4a", "ogg", "oga", "opus", "wma", "alac", "ape",
    "wv", "amr", "au", "mid", "midi", "aifc", "caf",
];

pub const DOC_EXTS: &[&str] = &["pdf", "ps"];
pub const OFFICE_EXTS: &[&str] = &[
    "docx", "xlsx", "pptx", "docm", "xlsm", "pptm", "odt", "ods", "odp",
];
pub const ARCHIVE_EXTS: &[&str] = &[
    "zip", "jar", "whl", "egg", "apk", "epub", "xpi", "cbz", "zipx",
];
pub const TEXT_EXTS: &[&str] = &[
    "txt",
    "log",
    "csv",
    "tsv",
    "json",
    "xml",
    "html",
    "htm",
    "css",
    "js",
    "mjs",
    "ts",
    "jsx",
    "tsx",
    "py",
    "rb",
    "php",
    "pl",
    "lua",
    "c",
    "h",
    "cpp",
    "hpp",
    "cc",
    "cs",
    "java",
    "kt",
    "swift",
    "go",
    "rs",
    "sql",
    "yaml",
    "yml",
    "toml",
    "ini",
    "cfg",
    "conf",
    "md",
    "rst",
    "tex",
    "bat",
    "ps1",
    "sh",
    "gitignore",
    "env",
    "properties",
    "gradle",
];
pub const FONT_EXTS: &[&str] = &["ttf", "otf", "woff", "woff2", "eot"];
pub const LOSSLESS_EXTS: &[&str] = &[
    "exe", "dll", "so", "dylib", "a", "lib", "o", "obj", "bin", "dat", "iso", "img", "ttc", "srt",
    "vtt", "ass", "ssa", "sub", "ics", "vcf", "kmz", "wasm", "map", "rtf", "dxf", "ply", "stl",
    "geojson", "kml", "gpx",
];

pub const ALREADY_COMPRESSED_EXTS: &[&str] = &[
    "jpg", "jpeg", "jpe", "jfif", "mp3", "aac", "m4a", "ogg", "opus", "wma", "mp4", "m4v", "mkv",
    "webm", "avi", "mov", "flv", "wmv", "mpg", "mpeg", "3gp", "ts", "m2ts", "zip", "rar", "7z",
    "gz", "bz2", "xz", "zst", "br", "lz4", "lzma", "apk", "jar", "whl", "docx", "xlsx", "pptx",
    "odt", "ods", "odp", "epub", "webp", "avif", "heic", "heif", "jxl", "cab", "deb", "rpm", "msi",
    "dmg", "woff", "woff2", "lz", "tlz", "txz", "tbz", "arj", "lzh", "z", "sit", "sitx", "pak",
    "vmdk", "mp2", "m2v", "vob", "rm", "rmvb",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KindCount {
    pub kind: String,
    pub files: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub files: u64,
    pub bytes: u64,
    pub counts: Vec<KindCount>,
    pub skipped: u64,
    pub source: String,
    pub destination: String,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub src: PathBuf,
    pub rel: PathBuf,
    pub kind: String,
    pub size: u64,
    pub preferred_ext: String,
}

fn ext_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}

fn has(list: &[&str], ext: &str) -> bool {
    list.contains(&ext)
}

fn contains_chunk(path: &Path, marker: &[u8]) -> bool {
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut buf = vec![0u8; 64 * 1024];
    let Ok(read) = file.read(&mut buf) else {
        return false;
    };
    buf[..read]
        .windows(marker.len())
        .any(|window| window == marker)
}

pub fn is_animated(path: &Path) -> bool {
    match ext_of(path).as_str() {
        "gif" => true,
        "webp" => contains_chunk(path, b"ANMF") || contains_chunk(path, b"ANIM"),
        "png" | "apng" => contains_chunk(path, b"acTL"),
        _ => false,
    }
}

fn image_target(ext: &str, settings: &Settings, ffmpeg: bool) -> String {
    let mode = settings.image_format.as_str();
    let mut target = match mode {
        "jpg" => "jpg",
        "webp" => "webp",
        "avif" => "avif",
        "png" => "png",
        "jxl" => "jxl",
        "keep" => ext,
        _ => match ext {
            "jpg" | "jpeg" | "jpe" | "jfif" => "jpg",
            "png" => "png",
            "bmp" | "tif" | "tiff" | "ppm" | "pgm" | "pnm" | "jp2" | "j2k" | "heic" | "heif"
            | "hdr" | "exr" => "jpg",
            "webp" => "webp",
            "pbm" | "pcx" | "tga" | "dds" | "ico" | "cur" | "qoi" => "png",
            "jxl" => "webp",
            "avif" => "avif",
            _ => "jpg",
        },
    };

    if settings.convert_png_to_jpeg && ext == "png" && matches!(mode, "auto" | "keep" | "png") {
        target = "jpg";
    }
    if matches!(ext, "bmp" | "tif" | "tiff")
        && settings.convert_bmp_to_jpeg
        && matches!(mode, "auto" | "keep")
    {
        target = "jpg";
    }
    if target == "avif" && !ffmpeg {
        target = "webp";
    }
    if target == "jxl" {
        target = "webp";
    }
    target.to_string()
}

fn classify_video(ext: &str, settings: &Settings, ffmpeg: bool) -> (String, String) {
    if !ffmpeg {
        return ("copy".into(), ext.to_string());
    }
    let out = if settings.video_codec == "copy" {
        if matches!(ext, "mp4" | "mkv" | "webm") {
            ext
        } else {
            "mkv"
        }
    } else {
        match ext {
            "mkv" => "mkv",
            "webm" => "webm",
            _ => "mp4",
        }
    };
    ("video".into(), out.into())
}

fn classify_audio(ext: &str, settings: &Settings, ffmpeg: bool) -> (String, String) {
    if settings.video_audio_codec == "copy" || matches!(ext, "mid" | "midi" | "amr") || !ffmpeg {
        return ("copy".into(), ext.to_string());
    }
    let default_ext = match ext {
        "aac" | "m4a" | "alac" | "caf" => "m4a",
        "ogg" | "oga" => "ogg",
        "opus" => "opus",
        "wma" => "wma",
        "flac" => "flac",
        _ => "mp3",
    };
    let out = match settings.video_audio_codec.as_str() {
        "opus" => "opus",
        "mp3" => "mp3",
        "aac" => "m4a",
        "vorbis" => "ogg",
        "flac" => "flac",
        _ => default_ext,
    };
    ("audio".into(), out.into())
}

fn maybe_copy(kind: &str, mode: &str, ext: &str) -> (String, String) {
    let out_kind = if matches!(mode, "keep" | "copy") {
        "copy"
    } else {
        kind
    };
    (out_kind.into(), ext.to_string())
}

fn classify_other(ext: &str, settings: &Settings) -> (String, String) {
    if has(DOC_EXTS, ext) {
        return maybe_copy("pdf", &settings.pdf_mode, ext);
    }
    if has(OFFICE_EXTS, ext) {
        return maybe_copy("office", &settings.office_mode, ext);
    }
    if ext == "svg" {
        return maybe_copy("svg", &settings.svg_mode, ext);
    }
    if has(FONT_EXTS, ext) {
        return ("font".into(), ext.to_string());
    }
    if has(ARCHIVE_EXTS, ext) {
        return maybe_copy("archive", &settings.archive_mode, ext);
    }
    if has(TEXT_EXTS, ext) {
        return maybe_copy("text", &settings.text_mode, ext);
    }
    if has(LOSSLESS_EXTS, ext) && !has(ALREADY_COMPRESSED_EXTS, ext) {
        return maybe_copy("lossless", &settings.text_mode, ext);
    }
    ("copy".into(), ext.to_string())
}

pub fn classify(path: &Path, settings: &Settings, ffmpeg: bool) -> (String, String) {
    let ext = ext_of(path);
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if matches!(
        name.as_str(),
        "thumbs.db" | ".ds_store" | "desktop.ini" | "ehthumbs.db"
    ) {
        return ("copy".into(), ext);
    }

    if has(VIDEO_EXTS, &ext) {
        return classify_video(&ext, settings, ffmpeg);
    }

    if ext == "gif"
        || has(&["apng", "flif"], &ext)
        || (has(&["webp", "png"], &ext) && is_animated(path))
    {
        if matches!(settings.gif_mode.as_str(), "keep" | "copy") || (!ffmpeg && ext != "gif") {
            return ("copy".into(), ext);
        }
        let out = match settings.gif_mode.as_str() {
            "gif" => "gif",
            "webp" => "webp",
            _ => "mp4",
        };
        return ("gif".into(), out.into());
    }

    if has(AUDIO_EXTS, &ext) {
        return classify_audio(&ext, settings, ffmpeg);
    }

    if has(IMAGE_EXTS, &ext) || has(FFMPEG_IMAGE_EXTS, &ext) {
        if has(FFMPEG_IMAGE_EXTS, &ext) && !ffmpeg {
            return ("copy".into(), ext);
        }
        return ("image".into(), image_target(&ext, settings, ffmpeg));
    }

    classify_other(&ext, settings)
}

pub fn wanted_kinds(settings: &Settings) -> Vec<String> {
    settings
        .only_kinds
        .iter()
        .map(|k| k.trim().to_lowercase())
        .filter(|k| !k.is_empty())
        .collect()
}

fn build_globs(patterns: &str) -> Option<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    let mut any = false;
    for pattern in patterns.split(',').flat_map(|p| p.split(';')) {
        let pattern = pattern.trim();
        if pattern.is_empty() {
            continue;
        }
        if let Ok(glob) = Glob::new(pattern) {
            builder.add(glob);
            any = true;
        }
    }
    if any { builder.build().ok() } else { None }
}

fn is_hidden_system(entry: &DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy().to_lowercase();
    matches!(
        name.as_str(),
        "system volume information" | "$recycle.bin" | "recycler" | ".git" | "node_modules"
    )
}

struct WalkFilter<'a> {
    globs: Option<GlobSet>,
    wanted: Vec<String>,
    min_size: u64,
    max_size: u64,
    settings: &'a Settings,
    ffmpeg: bool,
}

impl<'a> WalkFilter<'a> {
    fn new(settings: &'a Settings, ffmpeg: bool) -> Self {
        Self {
            globs: build_globs(&settings.exclude_globs),
            wanted: wanted_kinds(settings),
            min_size: settings.min_size_kb.saturating_mul(1024),
            max_size: settings.limit_mb.saturating_mul(1024 * 1024),
            settings,
            ffmpeg,
        }
    }
}

fn walk_files(src: &Path, dst: &Path) -> impl Iterator<Item = DirEntry> {
    let dst = dst.to_path_buf();
    WalkDir::new(src)
        .follow_links(false)
        .into_iter()
        .filter_entry(move |entry| {
            if entry.depth() == 0 {
                return true;
            }
            if entry.file_type().is_dir() {
                if entry.path() == dst || entry.path().starts_with(&dst) {
                    return false;
                }
                if is_hidden_system(entry) {
                    return false;
                }
            }
            true
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
}

fn build_task(entry: &DirEntry, src: &Path, filter: &WalkFilter<'_>) -> Option<Task> {
    let path = entry.path();
    let rel = path.strip_prefix(src).unwrap_or(path);
    let rel_text = rel.to_string_lossy().replace('\\', "/");
    if let Some(globs) = &filter.globs
        && (globs.is_match(&rel_text) || globs.is_match(path.file_name().unwrap_or_default()))
    {
        return None;
    }
    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
    if (filter.min_size > 0 && size < filter.min_size)
        || (filter.max_size > 0 && size > filter.max_size)
    {
        return None;
    }
    let (mut kind, preferred_ext) = classify(path, filter.settings, filter.ffmpeg);
    if !filter.wanted.is_empty() && kind != "copy" && !filter.wanted.contains(&kind) {
        if filter.settings.only_strict {
            return None;
        }
        kind = "copy".into();
    }
    Some(Task {
        src: path.to_path_buf(),
        rel: rel.to_path_buf(),
        kind,
        size,
        preferred_ext,
    })
}

pub fn scan(src: &Path, dst: &Path, settings: &Settings, ffmpeg: bool) -> Result<ScanResult> {
    let filter = WalkFilter::new(settings, ffmpeg);
    let mut counts: HashMap<String, (u64, u64)> = HashMap::new();
    let mut tasks = Vec::new();
    let mut skipped = 0u64;

    for entry in walk_files(src, dst) {
        let Some(task) = build_task(&entry, src, &filter) else {
            skipped += 1;
            continue;
        };
        let entry = counts.entry(task.kind.clone()).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += task.size;
        tasks.push(task);
    }

    let mut counts: Vec<KindCount> = counts
        .into_iter()
        .map(|(kind, (files, bytes))| KindCount { kind, files, bytes })
        .collect();
    counts.sort_by_key(|a| std::cmp::Reverse(a.files));

    Ok(ScanResult {
        files: tasks.len() as u64,
        bytes: tasks.iter().map(|t| t.size).sum(),
        skipped,
        counts,
        source: src.display().to_string(),
        destination: dst.display().to_string(),
    })
}

pub fn collect_tasks(
    src: &Path,
    dst: &Path,
    settings: &Settings,
    ffmpeg: bool,
) -> Result<Vec<Task>> {
    let filter = WalkFilter::new(settings, ffmpeg);
    Ok(walk_files(src, dst)
        .filter_map(|entry| build_task(&entry, src, &filter))
        .collect())
}

pub fn kind_label(kind: &str) -> &'static str {
    match kind {
        "image" => "Image",
        "video" => "Video",
        "audio" => "Audio",
        "gif" => "GIF/Animation",
        "pdf" => "PDF",
        "office" => "Office",
        "archive" => "Archive",
        "text" => "Text/Code",
        "svg" => "SVG",
        "font" => "Font",
        "lossless" => "Lossless",
        _ => "Copy",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_respects_keep_and_copy_modes() {
        let settings = Settings {
            gif_mode: "keep".into(),
            video_audio_codec: "copy".into(),
            text_mode: "keep".into(),
            pdf_mode: "copy".into(),
            office_mode: "keep".into(),
            svg_mode: "copy".into(),
            archive_mode: "keep".into(),
            ..Default::default()
        };

        assert_eq!(
            classify(Path::new("anim.gif"), &settings, true),
            ("copy".into(), "gif".into())
        );
        assert_eq!(
            classify(Path::new("song.mp3"), &settings, true),
            ("copy".into(), "mp3".into())
        );
        assert_eq!(
            classify(Path::new("notes.txt"), &settings, true),
            ("copy".into(), "txt".into())
        );
        assert_eq!(
            classify(Path::new("module.wasm"), &settings, true),
            ("copy".into(), "wasm".into())
        );
        assert_eq!(
            classify(Path::new("doc.pdf"), &settings, true),
            ("copy".into(), "pdf".into())
        );
        assert_eq!(
            classify(Path::new("deck.pptx"), &settings, true),
            ("copy".into(), "pptx".into())
        );
        assert_eq!(
            classify(Path::new("icon.svg"), &settings, true),
            ("copy".into(), "svg".into())
        );
        assert_eq!(
            classify(Path::new("bundle.zip"), &settings, true),
            ("copy".into(), "zip".into())
        );
    }
}
