use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use crate::content::ContentKind;

const MAX_ICON: u64 = 1024 * 1024;
const MAX_MANIFEST: u64 = 256 * 1024;
const FABRIC_MANIFEST: &str = "fabric.mod.json";
const PACK_MANIFEST: &str = "pack.mcmeta";
const PACK_ICON: &str = "pack.png";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocalMetadata {
    pub id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub icon: Option<Arc<[u8]>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Person {
    Name(String),
    Detailed { name: String },
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Icon {
    Path(String),
    Sized(std::collections::BTreeMap<String, String>),
}

#[derive(Deserialize)]
struct FabricManifest {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    authors: Vec<Person>,
    #[serde(default)]
    icon: Option<Icon>,
}

#[derive(Deserialize)]
struct PackManifest {
    pack: PackSection,
}

#[derive(Deserialize)]
struct PackSection {
    #[serde(default)]
    description: Value,
}

fn entry<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
    limit: u64,
) -> Option<Vec<u8>> {
    let file = archive.by_name(name).ok()?;
    if file.size() > limit {
        return None;
    }
    let mut bytes = Vec::with_capacity(file.size() as usize);
    file.take(limit).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

fn flatten(component: &Value, out: &mut String) {
    match component {
        Value::String(text) => out.push_str(text),
        Value::Array(parts) => parts.iter().for_each(|part| flatten(part, out)),
        Value::Object(fields) => {
            if let Some(text) = fields.get("text") {
                flatten(text, out);
            }
            if let Some(extra) = fields.get("extra") {
                flatten(extra, out);
            }
        }
        _ => {}
    }
}

fn icon_path(icon: Icon) -> Option<String> {
    match icon {
        Icon::Path(path) => Some(path),
        Icon::Sized(sizes) => sizes
            .into_iter()
            .max_by_key(|(size, _)| size.parse::<u32>().unwrap_or(0))
            .map(|(_, path)| path),
    }
}

pub(crate) fn read_metadata(kind: ContentKind, path: &Path) -> Option<LocalMetadata> {
    let file = File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    match kind {
        ContentKind::Mod => {
            let manifest: FabricManifest =
                serde_json::from_slice(&entry(&mut archive, FABRIC_MANIFEST, MAX_MANIFEST)?)
                    .ok()?;
            let icon = manifest
                .icon
                .and_then(icon_path)
                .and_then(|icon| entry(&mut archive, icon.trim_start_matches('/'), MAX_ICON));
            Some(LocalMetadata {
                id: manifest.id,
                name: manifest.name,
                version: manifest.version,
                description: manifest.description,
                authors: manifest
                    .authors
                    .into_iter()
                    .map(|person| match person {
                        Person::Name(name) | Person::Detailed { name } => name,
                    })
                    .collect(),
                icon: icon.map(Arc::from),
            })
        }
        ContentKind::ResourcePack | ContentKind::ShaderPack | ContentKind::DataPack => {
            let description = entry(&mut archive, PACK_MANIFEST, MAX_MANIFEST)
                .and_then(|bytes| serde_json::from_slice::<PackManifest>(&bytes).ok())
                .map(|manifest| {
                    let mut text = String::new();
                    flatten(&manifest.pack.description, &mut text);
                    text
                })
                .filter(|text| !text.is_empty());
            let icon = entry(&mut archive, PACK_ICON, MAX_ICON);
            Some(LocalMetadata {
                description,
                icon: icon.map(Arc::from),
                ..LocalMetadata::default()
            })
        }
    }
}
