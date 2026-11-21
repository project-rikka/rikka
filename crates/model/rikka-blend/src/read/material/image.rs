use std::path::Path;
use std::sync::Arc;

use rikka_model_core::{Error, Result};

use super::generated::{Generated, generated};
use crate::file::Record;
use crate::read::modifier::Notes;
use crate::read::resources::Resources;

const SOURCE_GENERATED: i64 = 4;
const SOURCE_VIEWER: i64 = 5;
const SOURCE_TILED: i64 = 6;

pub(crate) struct LoadedImage {
    pub(crate) name: String,
    pub(crate) bytes: Option<Arc<[u8]>>,
}

pub(crate) fn load(
    image: Record<'_>,
    resources: &mut dyn Resources,
    notes: &mut Notes,
) -> Result<Option<LoadedImage>> {
    let id_name = image.id_name();
    let source = image.int("source").unwrap_or(1);
    match source {
        SOURCE_VIEWER => {
            notes.unsupported(format!("viewer image \"{id_name}\" has no stored pixels"));
            return Ok(None);
        }
        SOURCE_GENERATED => {
            return match generated(&image) {
                Generated::Png(bytes) => {
                    resources.charge(bytes.len() as u64)?;
                    Ok(Some(LoadedImage {
                        name: format!("{id_name}.png"),
                        bytes: Some(bytes.into()),
                    }))
                }
                Generated::Unsupported => {
                    notes.unsupported(format!(
                        "generated image \"{id_name}\" uses an unsupported pattern"
                    ));
                    Ok(None)
                }
            };
        }
        _ => {}
    }
    let stored = image
        .text("name")
        .or_else(|| image.text("filepath"))
        .unwrap_or_default();
    let path = stored.replace('\\', "/");
    let path = match source {
        SOURCE_TILED => path.replace("<UDIM>", "1001").replace("<UVTILE>", "u1_v1"),
        _ => path,
    };
    let file_name = Path::new(&path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| id_name.clone());

    if let Some(bytes) = packed(&image) {
        resources.charge(bytes.len() as u64)?;
        return Ok(Some(LoadedImage {
            name: file_name,
            bytes: Some(bytes.into()),
        }));
    }
    if path.is_empty() {
        notes.unsupported(format!("image \"{id_name}\" has no file path"));
        return Ok(None);
    }
    let dir = image.blend().folder().to_owned();
    let mut candidates: Vec<String> = Vec::new();
    match path.strip_prefix("//") {
        Some(relative) => candidates.push(format!("{dir}{relative}")),
        None if !path.starts_with('/') && !path.contains(':') => {
            candidates.push(format!("{dir}{path}"))
        }
        None => {}
    }
    candidates.push(format!("{dir}textures/{file_name}"));
    candidates.push(format!("{dir}{file_name}"));
    candidates.dedup();
    let Some((name, earlier)) = candidates.split_last() else {
        return Ok(None);
    };
    let mut bytes = None;
    for uri in earlier.iter().filter(|uri| *uri != name) {
        match resources.read_resource(uri) {
            Ok(found) => {
                bytes = Some(found);
                break;
            }
            Err(Error::Read { .. }) => {}
            Err(error) => return Err(error),
        }
    }
    let bytes = match bytes {
        Some(bytes) => Some(bytes),
        None => resources.read_optional_resource(name)?,
    };
    Ok(Some(LoadedImage {
        name: name.clone(),
        bytes,
    }))
}

fn packed(image: &Record<'_>) -> Option<Vec<u8>> {
    let file = image.deref("packedfile").or_else(|| {
        image
            .list("packedfiles")
            .first()
            .and_then(|entry| entry.deref("packedfile"))
    })?;
    let size = file.int("size").unwrap_or(0).max(0) as usize;
    let bytes = file.raw("data")?;
    Some(bytes[..size.min(bytes.len())].to_vec())
}
