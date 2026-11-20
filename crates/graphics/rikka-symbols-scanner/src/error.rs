use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "{0}\nusage: rikka-symbols-scanner <material|brands> <listing> <font> <output.rs>\n       rikka-symbols-scanner symbolic <listing> <font> <icon> <output.svg>\n  material lists a Material Symbols .codepoints file, brands a Font Awesome icons.json;\n  symbolic draws one listed Material Symbol, filled, as a symbolic SVG icon"
    )]
    Usage(String),
    #[error("failed to read {path}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("failed to write {path}: {source}")]
    Write { path: PathBuf, source: io::Error },
    #[error("{path}:{line}: {message}")]
    Listing {
        path: PathBuf,
        line: usize,
        message: String,
    },
    #[error("{path} is not a Font Awesome icon listing: {source}")]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("icon `{name}` has invalid codepoint {value:?}")]
    Codepoint { name: String, value: String },
    #[error("icon name `{0}` is not a valid Rust identifier")]
    InvalidName(String),
    #[error("icons `{first}` and `{second}` both map to constant {constant}")]
    ConstantCollision {
        constant: String,
        first: String,
        second: String,
    },
    #[error("{path} is not a font: {source}")]
    Font {
        path: PathBuf,
        source: rikka_text_font::Error,
    },
    #[error("{path} has no glyph for {} listed icons: {}", missing.len(), missing.join(", "))]
    MissingGlyphs { path: PathBuf, missing: Vec<String> },
    #[error("{listing} has no icon named `{name}`")]
    UnknownIcon { name: String, listing: PathBuf },
    #[error("{path} has no outline for `{name}`")]
    MissingOutline { path: PathBuf, name: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
