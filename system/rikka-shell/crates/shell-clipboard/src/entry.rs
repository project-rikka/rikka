use std::sync::Arc;

use rikka_clipboard::{Offer, TEXT};

const PREVIEW: usize = 140;

#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub(crate) id: u64,
    pub(crate) content: Content,
    pub(crate) preview: Preview,
    pub(crate) pinned: bool,
    key: Option<Arc<str>>,
}

impl Entry {
    pub(crate) fn new(id: u64, content: Content) -> Entry {
        Entry {
            id,
            preview: Preview::from_content(&content),
            key: key_of(&content),
            content,
            pinned: false,
        }
    }

    pub(crate) fn matches(&self, query: &str) -> bool {
        self.key.as_ref().is_some_and(|key| key.contains(query))
    }

    pub(crate) fn weight(&self) -> usize {
        self.content.weight() + self.key.as_ref().map_or(0, |key| key.len())
    }
}

fn key_of(content: &Content) -> Option<Arc<str>> {
    match content {
        Content::Text(text) => Some(Arc::from(text.to_lowercase().as_str())),
        Content::Files(uris) => Some(Arc::from(uris.join("\n").to_lowercase().as_str())),
        Content::Image(_) => None,
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Preview {
    pub(crate) title: String,
    pub(crate) rest: Option<String>,
}

impl Preview {
    fn from_content(content: &Content) -> Preview {
        match content {
            Content::Text(body) => {
                let mut lines = body.lines().filter(|line| !line.trim().is_empty());
                let title = joined(lines.next().unwrap_or_default().split_whitespace(), " ");
                let rest = joined(lines.flat_map(str::split_whitespace), " ");
                Preview {
                    title,
                    rest: (!rest.is_empty()).then_some(rest),
                }
            }
            Content::Files(uris) => Preview {
                title: joined(uris.iter().map(|uri| name_of(uri)), ", "),
                rest: None,
            },
            Content::Image(_) => Preview::default(),
        }
    }
}

fn joined<S: AsRef<str>>(parts: impl Iterator<Item = S>, between: &str) -> String {
    let mut line = String::new();
    for part in parts {
        if line.chars().count() >= PREVIEW {
            break;
        }
        if !line.is_empty() {
            line.push_str(between);
        }
        line.push_str(part.as_ref());
    }
    if let Some((at, _)) = line.char_indices().nth(PREVIEW) {
        line.truncate(at);
    }
    line
}

fn name_of(uri: &str) -> String {
    let tail = uri.rsplit('/').next().unwrap_or(uri);
    rikka_open::decode_path(tail).to_string_lossy().into_owned()
}

#[derive(Clone, Debug)]
pub(crate) enum Content {
    Text(Arc<str>),
    Files(Vec<String>),
    Image(Picture),
}

#[derive(Clone, Debug)]
pub(crate) struct Picture {
    pub(super) mime: String,
    pub(crate) data: Arc<[u8]>,
    pub(crate) thumbnail: Arc<[u8]>,
    pub(super) thumbnail_width: u32,
    pub(super) thumbnail_height: u32,
}

impl Content {
    pub(crate) fn offer(&self) -> Offer {
        match self {
            Content::Text(text) => Offer::text(text),
            Content::Files(uris) => {
                Offer::uris(uris).with(TEXT, Arc::<[u8]>::from(uris.join("\n").as_bytes()))
            }
            Content::Image(picture) => Offer::new().with(&picture.mime, picture.data.clone()),
        }
    }

    pub(crate) fn weight(&self) -> usize {
        match self {
            Content::Text(text) => text.len(),
            Content::Files(uris) => uris.iter().map(String::len).sum(),
            Content::Image(picture) => picture.data.len() + picture.thumbnail.len(),
        }
    }

    pub(crate) fn is(&self, other: &Content) -> bool {
        match (self, other) {
            (Content::Text(ours), Content::Text(theirs)) => ours == theirs,
            (Content::Files(ours), Content::Files(theirs)) => ours == theirs,
            (Content::Image(ours), Content::Image(theirs)) => {
                ours.mime == theirs.mime && ours.data == theirs.data
            }
            _ => false,
        }
    }
}
