use std::path::{Component, Path, PathBuf};

use paint_document::FileFormat;
use serde::Deserialize;
use serde_json::Value as Json;

use super::read;
use crate::ops::Parsed;
use crate::order::{Order, Request, SaveOrder};
use crate::vocabulary::formats;

const QUALITIES: std::ops::RangeInclusive<u8> = 1..=100;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveArgs {
    #[serde(default)]
    document: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    quality: Option<u8>,
}

pub(crate) fn save(input: Json, home: Option<&Path>) -> Parsed<Order> {
    let args: SaveArgs = read(input)?;
    let path = args
        .path
        .map(|path| writable(Path::new(&path), home))
        .transpose()?;
    if let Some(quality) = args.quality
        && !QUALITIES.contains(&quality)
    {
        return Err(format!("quality is {quality}; use 1–100"));
    }
    Ok(Order {
        document: args.document,
        request: Request::Save(SaveOrder {
            path,
            quality: args.quality,
        }),
    })
}

pub(crate) fn writable(path: &Path, home: Option<&Path>) -> Parsed<PathBuf> {
    let displayed = path.display();
    let home = home.ok_or("there is no home folder to save into")?;
    if !path.is_absolute() {
        return Err(format!(
            "{displayed} is not absolute; give the whole path, inside {}",
            home.display()
        ));
    }
    if path
        .components()
        .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    {
        return Err(format!(
            "{displayed} climbs with . or ..; give a plain path"
        ));
    }
    if !path.starts_with(home) || path == home {
        return Err(format!(
            "{displayed} lies outside the home folder; save inside {}",
            home.display()
        ));
    }
    if FileFormat::for_path(path).is_none() {
        return Err(format!(
            "{displayed} has no extension Paint writes; use {}",
            formats()
        ));
    }
    if path.is_dir() {
        return Err(format!("{displayed} is a folder"));
    }
    Ok(path.to_path_buf())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenArgs {
    path: String,
}

pub(crate) fn open(input: Json) -> Parsed<Order> {
    let args: OpenArgs = read(input)?;
    let path = PathBuf::from(&args.path);
    if !path.is_absolute() {
        return Err(format!(
            "{} is not absolute; give the whole path",
            args.path
        ));
    }
    if !path.is_file() {
        return Err(format!("there is no file at {}", args.path));
    }
    Ok(Order {
        document: None,
        request: Request::Open(path),
    })
}
