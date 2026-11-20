use crate::id::{ChannelId, EmojiId, RoleId, UserId};
use crate::model::picture::Picture;

const CDN: &str = "https://cdn.discordapp.com";
const PICTURE_SIZE: &str = "size";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rich(pub Vec<Block>);

impl Rich {
    #[must_use]
    pub fn plain(text: impl Into<String>) -> Rich {
        let text = text.into();
        if text.is_empty() {
            return Rich::default();
        }
        Rich(vec![Block::Paragraph(vec![Inline::Text(text)])])
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn only_emoji(&self) -> bool {
        let mut emoji = 0;
        for block in &self.0 {
            let Block::Paragraph(inlines) = block else {
                return false;
            };
            for inline in inlines {
                match inline {
                    Inline::Emoji(_) => emoji += 1,
                    Inline::Text(text) if text.trim().is_empty() => {}
                    _ => return false,
                }
            }
        }
        (1..=30).contains(&emoji)
    }

    #[must_use]
    pub fn plain_text(&self) -> String {
        let mut out = String::new();
        for (at, block) in self.0.iter().enumerate() {
            if at > 0 {
                out.push('\n');
            }
            block.write_plain(&mut out);
        }
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Paragraph(Vec<Inline>),
    Heading {
        level: u8,
        content: Vec<Inline>,
    },
    Subtext(Vec<Inline>),
    Quote(Vec<Block>),
    Code {
        language: Option<String>,
        text: String,
    },
    List {
        start: Option<u32>,
        items: Vec<Vec<Block>>,
    },
}

impl Block {
    fn write_plain(&self, out: &mut String) {
        match self {
            Block::Paragraph(inlines)
            | Block::Subtext(inlines)
            | Block::Heading {
                content: inlines, ..
            } => write_inlines(inlines, out),
            Block::Quote(blocks) => {
                for (at, block) in blocks.iter().enumerate() {
                    if at > 0 {
                        out.push('\n');
                    }
                    block.write_plain(out);
                }
            }
            Block::Code { text, .. } => out.push_str(text),
            Block::List { items, .. } => {
                for (at, item) in items.iter().enumerate() {
                    if at > 0 {
                        out.push('\n');
                    }
                    for block in item {
                        block.write_plain(out);
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    Bold(Vec<Inline>),
    Italic(Vec<Inline>),
    Underline(Vec<Inline>),
    Strike(Vec<Inline>),
    Spoiler(Vec<Inline>),
    Code(String),
    Link { url: String, label: Vec<Inline> },
    Mention(Mention),
    Emoji(Emoji),
    Time { unix: i64, style: TimeStyle },
    Break,
}

fn write_inlines(inlines: &[Inline], out: &mut String) {
    for inline in inlines {
        match inline {
            Inline::Text(text) | Inline::Code(text) => out.push_str(text),
            Inline::Bold(inner)
            | Inline::Italic(inner)
            | Inline::Underline(inner)
            | Inline::Strike(inner)
            | Inline::Spoiler(inner)
            | Inline::Link { label: inner, .. } => write_inlines(inner, out),
            Inline::Mention(_) => out.push('@'),
            Inline::Emoji(emoji) => match &emoji.id {
                Some(_) => {
                    out.push(':');
                    out.push_str(&emoji.name);
                    out.push(':');
                }
                None => out.push_str(&emoji.name),
            },
            Inline::Time { .. } => out.push('⏲'),
            Inline::Break => out.push('\n'),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Mention {
    User(UserId),
    Role(RoleId),
    Channel(ChannelId),
    Everyone,
    Here,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Emoji {
    pub id: Option<EmojiId>,
    pub name: String,
    pub picture: Option<Picture>,
}

impl Emoji {
    #[must_use]
    pub fn unicode(glyph: impl Into<String>) -> Emoji {
        Emoji {
            id: None,
            name: glyph.into(),
            picture: None,
        }
    }

    #[must_use]
    pub fn custom(id: EmojiId, name: impl Into<String>, animated: bool) -> Emoji {
        let query = if animated { "?animated=true" } else { "" };
        let url = format!("{CDN}/emojis/{}.webp{query}", id.get());
        Emoji {
            id: Some(id),
            name: name.into(),
            picture: Some(Picture::sized(url, PICTURE_SIZE).animated(animated)),
        }
    }

    #[must_use]
    pub fn is_custom(&self) -> bool {
        self.id.is_some()
    }

    #[must_use]
    pub fn is_animated(&self) -> bool {
        self.picture.as_ref().is_some_and(Picture::is_animated)
    }

    #[must_use]
    pub fn same(&self, other: &Emoji) -> bool {
        match (&self.id, &other.id) {
            (Some(mine), Some(theirs)) => mine == theirs,
            (None, None) => self.name == other.name,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TimeStyle {
    ShortTime,
    LongTime,
    ShortDate,
    LongDate,
    #[default]
    ShortDateTime,
    LongDateTime,
    Relative,
}
