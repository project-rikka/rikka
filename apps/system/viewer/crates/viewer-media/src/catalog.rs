use std::path::Path;

use crate::format::Format;

const GLTF: Format = Format::model("glTF");
const GLB: Format = Format::model("glTF Binary");
const VRM: Format = Format::model("VRM");
const FBX: Format = Format::model("FBX");
const PMX: Format = Format::model("PMX");
const STL: Format = Format::model("STL");
const BLEND: Format = Format::model("Blender");
pub(crate) const VMD: Format = Format::model("VMD");

pub(crate) const PSD: Format = Format::layered("PSD");
pub(crate) const PSB: Format = Format::layered("PSB");

pub(crate) const TEXT: Format = Format::text("Text");
pub(crate) const MARKDOWN: Format = Format::text("Markdown");
const JSON: Format = Format::text("JSON");
const XML: Format = Format::text("XML");
const SVG: Format = Format::text("SVG");
const CSV: Format = Format::text("CSV");
const TOML: Format = Format::text("TOML");
const YAML: Format = Format::text("YAML");

const SIGNATURES: &[(&[u8], Format)] = &[
    (b"glTF", GLB),
    (b"Kaydara FBX Binary", FBX),
    (b"PMX ", PMX),
    (b"Vocaloid Motion Data", VMD),
    (b"BLENDER", BLEND),
    (b"8BPS\x00\x01", PSD),
    (b"8BPS\x00\x02", PSB),
];

const EXTENSIONS: &[(&str, Format)] = &[
    ("gltf", GLTF),
    ("glb", GLB),
    ("vrm", VRM),
    ("fbx", FBX),
    ("pmx", PMX),
    ("vmd", VMD),
    ("stl", STL),
    ("blend", BLEND),
    ("psd", PSD),
    ("psb", PSB),
    ("md", MARKDOWN),
    ("markdown", MARKDOWN),
    ("json", JSON),
    ("xml", XML),
    ("svg", SVG),
    ("csv", CSV),
    ("tsv", CSV),
    ("toml", TOML),
    ("yaml", YAML),
    ("yml", YAML),
    ("txt", TEXT),
    ("text", TEXT),
    ("log", TEXT),
    ("ini", TEXT),
    ("cfg", TEXT),
    ("conf", TEXT),
    ("desktop", TEXT),
    ("rs", TEXT),
    ("c", TEXT),
    ("h", TEXT),
    ("cc", TEXT),
    ("cpp", TEXT),
    ("cxx", TEXT),
    ("hpp", TEXT),
    ("py", TEXT),
    ("rb", TEXT),
    ("go", TEXT),
    ("java", TEXT),
    ("kt", TEXT),
    ("swift", TEXT),
    ("js", TEXT),
    ("mjs", TEXT),
    ("ts", TEXT),
    ("tsx", TEXT),
    ("jsx", TEXT),
    ("css", TEXT),
    ("html", TEXT),
    ("htm", TEXT),
    ("sh", TEXT),
    ("bash", TEXT),
    ("zsh", TEXT),
    ("fish", TEXT),
    ("lua", TEXT),
    ("sql", TEXT),
    ("patch", TEXT),
    ("diff", TEXT),
];

pub(crate) fn identify(path: &Path, header: &[u8]) -> Option<Format> {
    if let Some(format) = by_signature(header) {
        return Some(disambiguate(format, path));
    }
    match rikka_media_format::Format::identify(path, header) {
        Some(media) => Some(Format::from(media)),
        None => by_name(path),
    }
}

pub(crate) fn by_name(path: &Path) -> Option<Format> {
    let extension = path.extension()?.to_str()?;
    let by_extension = EXTENSIONS
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(extension))
        .map(|(_, format)| *format);
    by_extension.or_else(|| rikka_media_format::Format::from_path(path).map(Format::from))
}

fn by_signature(header: &[u8]) -> Option<Format> {
    SIGNATURES
        .iter()
        .find(|(bytes, _)| header.starts_with(bytes))
        .map(|(_, format)| *format)
}

fn disambiguate(format: Format, path: &Path) -> Format {
    let vrm = format == GLB
        && path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("vrm"));
    match vrm {
        true => VRM,
        false => format,
    }
}
