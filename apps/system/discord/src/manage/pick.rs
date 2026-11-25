use std::path::{Path, PathBuf};
use std::sync::Arc;

use discord_core::ImageUpload;
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse, Filter};
use rikka_image::Png;

const PNG: &str = "image/png";
const GIF: &str = "image/gif";
const MIMES: [(&str, &str); 9] = [
    ("png", PNG),
    ("apng", PNG),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", GIF),
    ("webp", "image/webp"),
    ("json", "application/json"),
    ("mp3", "audio/mpeg"),
    ("ogg", "audio/ogg"),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Picked {
    pub(crate) name: String,
    pub(crate) mime: String,
    pub(crate) bytes: Arc<[u8]>,
}

fn chosen_path(
    parent: Option<WindowIdentifier>,
    title: Label,
    mimes: &[&str],
) -> Result<Option<PathBuf>, String> {
    let filter = mimes.iter().fold(
        Filter::named(tr!("discord.manage.supported-files").to_string()),
        |filter, mime| filter.mime_type(*mime),
    );
    let selection = Chooser::open(title.to_string())
        .parent(parent)
        .filter(filter)
        .run()
        .map_err(|error| error.to_string())?;
    let ChooserResponse::Files(paths) = selection else {
        return Ok(None);
    };
    Ok(paths.into_iter().next())
}

fn mime_of(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    MIMES
        .iter()
        .find(|(known, _)| *known == extension)
        .map(|(_, mime)| *mime)
}

fn too_large(limit: u64) -> String {
    tr!(
        "discord.manage.file-too-large",
        limit = format!("{} KB", limit / 1024).as_str()
    )
    .to_string()
}

fn read(path: &Path, limit: u64) -> Result<Picked, String> {
    let failed =
        |error: &dyn std::fmt::Display| format!("failed to read {}: {error}", path.display());
    let mime = mime_of(path).ok_or_else(|| failed(&"unsupported file type"))?;
    let bytes = std::fs::read(path).map_err(|error| failed(&error))?;
    if bytes.len() as u64 > limit {
        return Err(too_large(limit));
    }
    let name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(Picked {
        name,
        mime: mime.to_owned(),
        bytes: bytes.into(),
    })
}

pub(crate) fn pick_file(
    parent: Option<WindowIdentifier>,
    title: Label,
    mimes: &[&str],
    limit: u64,
) -> Result<Option<Picked>, String> {
    chosen_path(parent, title, mimes)?
        .map(|path| read(&path, limit))
        .transpose()
}

pub(crate) fn pick_icon(parent: Option<WindowIdentifier>) -> Result<Option<ImageUpload>, String> {
    Ok(crate::space_sheet::chosen(parent)?.map(|png| ImageUpload::new(PNG, png)))
}

pub(crate) fn pick_picture(
    parent: Option<WindowIdentifier>,
    longest: u32,
    limit: u64,
) -> Result<Option<(String, ImageUpload)>, String> {
    let Some(path) = chosen_path(
        parent,
        tr!("discord.manage.choose-picture"),
        &[PNG, "image/jpeg", GIF, "image/webp"],
    )?
    else {
        return Ok(None);
    };
    let picked = read(&path, u64::MAX)?;
    let image = match picked.mime.as_str() {
        GIF => ImageUpload::new(GIF, picked.bytes),
        _ => {
            let fitted = fitted(&picked.bytes, longest)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
            ImageUpload::new(PNG, fitted)
        }
    };
    if image.bytes.len() as u64 > limit {
        return Err(too_large(limit));
    }
    Ok(Some((picked.name, image)))
}

fn fitted(bytes: &Arc<[u8]>, longest: u32) -> rikka_image::Result<Arc<[u8]>> {
    let picture = rikka_image::read(bytes).decode()?;
    let (width, height) = picture.dimensions();
    let scale = f64::from(longest) / f64::from(width.max(height));
    let resized = match scale < 1.0 {
        true => picture.resize_exact(
            ((f64::from(width) * scale).round() as u32).max(1),
            ((f64::from(height) * scale).round() as u32).max(1),
            rikka_image::Filter::Lanczos3,
        )?,
        false => picture,
    };
    Ok(resized.write(Png).to_bytes()?.into())
}
