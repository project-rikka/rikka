use rikka_agent::Reply;
use rikka_llm::{Image, Media};

use crate::contents::{Body, Contents};
use crate::part::Part;

#[derive(Debug, Default)]
pub(crate) struct Said {
    texts: Vec<String>,
    pictures: Vec<Image>,
}

impl Said {
    pub(crate) fn add(&mut self, part: Part) {
        match part {
            Part::Text(text) => self.texts.push(text),
            Part::Picture { media, data } => self.pictures.push(Image::inline(media, data)),
            Part::Contents(contents) => self.add_contents(contents),
        }
    }

    pub(crate) fn add_contents(&mut self, contents: Contents) {
        let media = contents.mime().and_then(Media::from_mime);
        let uri = contents.uri().to_owned();
        let kind = contents.mime().unwrap_or("data").to_owned();
        match contents.into_body() {
            Body::Text(text) => self.texts.push(format!("{uri}:\n{text}")),
            Body::Bytes(bytes) => match media.filter(|media| media.is_picture()) {
                Some(media) => {
                    self.texts.push(format!("{uri}: the picture below"));
                    self.pictures.push(Image::inline(media, bytes));
                }
                None => self.texts.push(format!(
                    "[{uri}: {} bytes of {kind}, which cannot be read here]",
                    bytes.len()
                )),
            },
        }
    }

    pub(crate) fn text(&self) -> String {
        self.texts.join("\n")
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.texts.iter().all(String::is_empty) && self.pictures.is_empty()
    }

    pub(crate) fn into_reply(self) -> Reply {
        let text = self.text();
        self.pictures
            .into_iter()
            .fold(Reply::new(text), Reply::image)
    }
}
