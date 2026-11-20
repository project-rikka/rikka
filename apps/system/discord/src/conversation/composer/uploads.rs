use std::path::PathBuf;
use std::sync::Arc;

use discord_core::{Limits, Upload, UploadSource};
use rikka_app::prelude::*;
use rikka_clipboard::{URI_LIST, uri_list};
use rikka_dialog::{Chooser, ChooserResponse};
use rikka_format::format_bytes;
use rikka_media_format::{Format, Kind};
use rikka_open::uri_to_path;
use rikka_ui::{Accept, DragOffer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Attach {
    Message,
    Post,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Pending {
    pub(crate) id: u64,
    pub(crate) upload: Upload,
}

impl Pending {
    pub(crate) fn is_picture(&self) -> bool {
        is_picture(&self.upload.name)
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct PickedFiles {
    pub(crate) uploads: Vec<Upload>,
    pub(crate) refused: Vec<String>,
}

pub(crate) fn is_picture(name: &str) -> bool {
    Format::from_path(name).is_some_and(|format| format.kind() == Kind::Picture)
}

pub(crate) fn admit(
    pending: &[Pending],
    picked: Vec<Upload>,
    limits: Limits,
    next: &mut u64,
) -> (Vec<Pending>, Vec<String>) {
    let mut admitted = Vec::new();
    let mut refused = Vec::new();
    for upload in picked {
        if pending.len() + admitted.len() >= limits.uploads_per_message {
            refused.push(
                tr!(
                    "discord.conversation.too-many-files",
                    count = limits.uploads_per_message as i64
                )
                .to_string(),
            );
            break;
        }
        if upload.size > limits.upload_bytes {
            refused.push(
                tr!(
                    "discord.conversation.too-large",
                    name = upload.name.as_str(),
                    size = format_bytes(limits.upload_bytes).as_str()
                )
                .to_string(),
            );
            continue;
        }
        *next += 1;
        admitted.push(Pending { id: *next, upload });
    }
    (admitted, refused)
}

pub(crate) fn chosen(parent: Option<WindowIdentifier>) -> PickedFiles {
    let selection = Chooser::open(tr!("discord.conversation.choose-files").to_string())
        .parent(parent)
        .accept_label(tr!("discord.conversation.attach").to_string())
        .multiple(true)
        .run();
    match selection {
        Ok(ChooserResponse::Files(paths)) => from_paths(paths),
        Ok(ChooserResponse::Cancelled) => PickedFiles::default(),
        Err(error) => PickedFiles {
            uploads: Vec::new(),
            refused: vec![error.to_string()],
        },
    }
}

pub(crate) fn picture_offered(offered: &[String]) -> Option<String> {
    offered
        .iter()
        .find(|mime| mime.starts_with("image/"))
        .cloned()
}

pub(crate) fn pasted(offered: Vec<String>) -> PickedFiles {
    if offered.iter().any(|mime| mime == URI_LIST)
        && let Ok(listed) = rikka_ui::clipboard::read(URI_LIST)
    {
        let paths: Vec<PathBuf> = uri_list(&String::from_utf8_lossy(&listed))
            .iter()
            .filter_map(|uri| uri_to_path(uri))
            .collect();
        if !paths.is_empty() {
            return from_paths(paths);
        }
    }
    let Some(mime) = picture_offered(&offered) else {
        return PickedFiles::default();
    };
    match rikka_ui::clipboard::read(&mime) {
        Ok(bytes) => from_bytes(&mime, bytes),
        Err(error) => PickedFiles {
            uploads: Vec::new(),
            refused: vec![error.to_string()],
        },
    }
}

pub(crate) fn accepts(offer: &DragOffer) -> Option<Accept> {
    if offer.mimes().iter().any(|mime| mime == URI_LIST) {
        return Some(Accept::copy(URI_LIST));
    }
    picture_offered(offer.mimes()).map(Accept::copy)
}

pub(crate) fn dropped(offer: &DragOffer) -> PickedFiles {
    let paths: Vec<PathBuf> = offer
        .uris()
        .iter()
        .filter_map(|uri| uri_to_path(uri))
        .collect();
    if !paths.is_empty() {
        return from_paths(paths);
    }
    let Some(mime) = picture_offered(offer.mimes()) else {
        return PickedFiles::default();
    };
    match offer.read(&mime) {
        Some(bytes) => from_bytes(&mime, bytes),
        None => PickedFiles::default(),
    }
}

fn from_bytes(mime: &str, bytes: Vec<u8>) -> PickedFiles {
    let extension = mime
        .strip_prefix("image/")
        .map(|kind| kind.split(';').next().unwrap_or(kind))
        .filter(|kind| !kind.is_empty())
        .unwrap_or("png");
    let size = bytes.len() as u64;
    PickedFiles {
        uploads: vec![Upload {
            name: format!("image.{extension}"),
            source: UploadSource::Bytes(Arc::from(bytes)),
            size,
            mime: Some(mime.to_owned()),
            description: None,
            spoiler: false,
        }],
        refused: Vec::new(),
    }
}

fn from_paths(paths: Vec<PathBuf>) -> PickedFiles {
    let mut picked = PickedFiles::default();
    for path in paths {
        match std::fs::metadata(&path) {
            Ok(metadata) if metadata.is_file() => picked.uploads.push(Upload {
                name: path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                size: metadata.len(),
                source: UploadSource::Path(path),
                mime: None,
                description: None,
                spoiler: false,
            }),
            Ok(_) => picked.refused.push(
                tr!(
                    "discord.conversation.not-a-file",
                    name = path.display().to_string().as_str()
                )
                .to_string(),
            ),
            Err(error) => picked.refused.push(error.to_string()),
        }
    }
    picked
}
