use std::path::Path;

use rikka_app::prelude::*;

pub(crate) fn folder(open: bool) -> MaterialSymbol {
    match open {
        true => MaterialSymbol::FOLDER_OPEN,
        false => MaterialSymbol::FOLDER,
    }
}

pub(crate) fn folder_tint() -> Color {
    Color::rgb(0.86, 0.69, 0.34)
}

pub(crate) fn file(path: &Path) -> MaterialSymbol {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if name.ends_with(".lock") || name == "flake.lock" {
        return MaterialSymbol::LOCK;
    }
    if name.starts_with(".env") {
        return MaterialSymbol::KEY;
    }
    if matches!(
        name.as_str(),
        "build" | "build.bazel" | "makefile" | "dockerfile" | "justfile" | "cargo.toml"
    ) {
        return MaterialSymbol::BUILD;
    }
    let extension = name.rsplit_once('.').map_or("", |(_, extension)| extension);
    match extension {
        "rs" | "c" | "h" | "cc" | "cpp" | "hpp" | "go" | "py" | "rb" | "java" | "kt" | "swift"
        | "cs" | "lua" | "php" | "zig" | "hs" | "ml" | "ex" | "exs" | "scala" | "dart" => {
            MaterialSymbol::CODE
        }
        "js" | "mjs" | "cjs" | "jsx" | "ts" | "mts" | "cts" | "tsx" => MaterialSymbol::JAVASCRIPT,
        "css" | "scss" | "sass" | "less" => MaterialSymbol::CSS,
        "html" | "htm" | "xhtml" | "vue" | "svelte" => MaterialSymbol::HTML,
        "sh" | "bash" | "zsh" | "fish" | "nu" => MaterialSymbol::TERMINAL,
        "json" | "jsonc" | "toml" | "yaml" | "yml" | "xml" | "ron" | "nix" | "bzl" | "ini"
        | "conf" | "cfg" => MaterialSymbol::DATA_OBJECT,
        "md" | "markdown" | "rst" | "adoc" => MaterialSymbol::ARTICLE,
        "txt" | "log" => MaterialSymbol::DESCRIPTION,
        "csv" | "tsv" | "sql" | "db" | "sqlite" => MaterialSymbol::TABLE_CHART,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "ico" | "bmp" | "avif" | "heic"
        | "heif" | "jxl" | "tif" | "tiff" | "exr" | "psd" => MaterialSymbol::IMAGE,
        "mp4" | "mkv" | "webm" | "mov" | "avi" => MaterialSymbol::MOVIE,
        "mp3" | "flac" | "ogg" | "wav" | "opus" | "m4a" => MaterialSymbol::AUDIO_FILE,
        "zip" | "tar" | "gz" | "xz" | "zst" | "bz2" | "7z" | "rar" => MaterialSymbol::FOLDER_ZIP,
        "pdf" => MaterialSymbol::PICTURE_AS_PDF,
        "ttf" | "otf" | "woff" | "woff2" => MaterialSymbol::FONT_DOWNLOAD,
        _ => MaterialSymbol::DRAFT,
    }
}
