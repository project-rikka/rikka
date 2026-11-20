use std::path::PathBuf;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rikka_seam::Version;
use rikka_seam::net::socket_path;
use rikka_tool::ImageSource;
use rikka_tool::{Image, Media, Reply};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const CAPABILITY: &str = "rikka.assist";

pub(crate) const VERSION: Version = Version::new(1, 0);

pub(crate) const DESCRIBE: &str = "describe";

pub(crate) const CALL: &str = "call";

pub(crate) const CANCEL: &str = "cancel";

pub(crate) const CHANGED: &str = "changed";

const FOLDER: &str = "assist";

pub(crate) const SOCKET_EXTENSION: &str = "sock";

pub(crate) const STOPPED: &str = "the call was stopped before it finished";

pub(crate) fn folder() -> Option<PathBuf> {
    Some(socket_path()?.parent()?.join(FOLDER))
}

pub(crate) const WINDOW: &str = "window";

pub(crate) const WINDOWS: &str = "Windows";

#[must_use]
pub fn exposed(kit: &str, tool: &str) -> String {
    format!("{kit}_{tool}")
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct Described {
    pub(crate) kit: String,
    pub(crate) title: String,
    pub(crate) about: String,
    pub(crate) instructions: String,
    #[serde(default)]
    pub(crate) tools: Vec<Declared>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Declared {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) schema: Value,
    #[serde(default)]
    pub(crate) gist: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Calling {
    pub(crate) call: String,
    pub(crate) tool: String,
    pub(crate) input: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Cancelling {
    pub(crate) call: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct Outcome {
    pub(crate) text: String,
    #[serde(default)]
    pub(crate) failed: bool,
    #[serde(default)]
    pub(crate) pictures: Vec<Picture>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Picture {
    pub(crate) media: String,
    pub(crate) data: String,
    #[serde(default, rename = "shown")]
    pub(crate) visible: bool,
}

impl Outcome {
    pub(crate) fn from_reply(result: Result<Reply, String>) -> Outcome {
        match result {
            Ok(reply) => {
                let pictures = reply
                    .images()
                    .iter()
                    .filter_map(|image| Picture::from_image(image, false))
                    .chain(
                        reply
                            .displayed_images()
                            .iter()
                            .filter_map(|image| Picture::from_image(image, true)),
                    )
                    .collect();
                Outcome {
                    text: reply.text().to_owned(),
                    failed: reply.is_failed(),
                    pictures,
                }
            }
            Err(reason) => Outcome {
                text: reason,
                failed: true,
                pictures: Vec::new(),
            },
        }
    }

    pub(crate) fn failed(reason: impl Into<String>) -> Outcome {
        Outcome::from_reply(Err(reason.into()))
    }

    pub(crate) fn into_reply(self) -> Reply {
        let mut reply = Reply::new(self.text);
        for picture in self.pictures {
            let visible = picture.visible;
            match picture.image() {
                Some(image) if visible => reply = reply.display(image),
                Some(image) => reply = reply.image(image),
                None => tracing::debug!("app sent an undecodable image"),
            }
        }
        match self.failed {
            true => reply.failed(),
            false => reply,
        }
    }
}

impl Picture {
    fn from_image(image: &Image, visible: bool) -> Option<Picture> {
        match image.source() {
            ImageSource::Inline { media, data } => Some(Picture {
                media: media.mime().to_owned(),
                data: STANDARD.encode(data),
                visible,
            }),
            ImageSource::Url(_) => None,
        }
    }

    fn image(self) -> Option<Image> {
        let media = Media::from_mime(&self.media)?;
        let data = STANDARD.decode(self.data).ok()?;
        Some(Image::inline(media, data))
    }
}
