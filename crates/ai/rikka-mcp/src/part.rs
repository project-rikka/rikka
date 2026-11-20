use crate::protocol::ContentBlock;
use rikka_llm::Media;

use crate::contents::Contents;
use crate::encoded::decoded;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Part {
    Text(String),
    Picture { media: Media, data: Vec<u8> },
    Contents(Contents),
}

impl Part {
    pub(crate) fn from_block(block: ContentBlock) -> Part {
        match block {
            ContentBlock::Text { text } => Part::Text(text),
            ContentBlock::Image { data, mime_type } => match pictured(&data, &mime_type) {
                Some(part) => part,
                None => Part::Text(format!(
                    "[a {mime_type} image, which cannot be looked at here]"
                )),
            },
            ContentBlock::Audio { mime_type } => Part::Text(format!(
                "[{mime_type} audio, which cannot be listened to here]"
            )),
            ContentBlock::Resource { resource } => match Contents::from_resource(resource) {
                Ok(contents) => Part::Contents(contents),
                Err(reason) => Part::Text(format!("[{reason}]")),
            },
            ContentBlock::ResourceLink {
                uri,
                name,
                description,
            } => Part::Text(match description {
                Some(description) => format!("[resource {uri}: {description}]"),
                None => format!("[resource {uri}: {name}]"),
            }),
            ContentBlock::Unknown => {
                Part::Text(String::from("[something this client cannot read]"))
            }
        }
    }
}

fn pictured(data: &str, mime: &str) -> Option<Part> {
    let media = Media::from_mime(mime).filter(|media| media.is_picture())?;
    Some(Part::Picture {
        media,
        data: decoded(data)?,
    })
}
