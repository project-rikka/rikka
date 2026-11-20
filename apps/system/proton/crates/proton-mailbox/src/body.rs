use std::sync::Arc;

use rikka_html::Document;
use rikka_markdown::markdown_text;
use rikka_proton_mail::{MULTIPART_MIXED, MimeMessage, MimeType, TEXT_HTML};

use crate::plain::Plain;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    Plain(Plain),
    Markdown(Markdown),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Markdown {
    source: Arc<str>,
    text: Arc<str>,
}

impl Markdown {
    #[must_use]
    pub fn from_html(html: &str) -> Markdown {
        let document = Document::parse(html);
        Markdown {
            source: document.to_markdown().into(),
            text: document.to_text().into(),
        }
    }

    #[must_use]
    pub fn source(&self) -> &Arc<str> {
        &self.source
    }
}

impl Body {
    #[must_use]
    pub fn text(&self) -> &Arc<str> {
        match self {
            Body::Plain(plain) => plain.text(),
            Body::Markdown(markdown) => &markdown.text,
        }
    }

    pub(crate) fn warm_markdown_cache(&self) {
        if let Body::Markdown(markdown) = self {
            drop(markdown_text(&markdown.source));
        }
    }
}

#[must_use]
pub fn decode(media_type: &MimeType, body: &[u8]) -> Body {
    match media_type.as_str() {
        TEXT_HTML => Body::Markdown(Markdown::from_html(&String::from_utf8_lossy(body))),
        MULTIPART_MIXED => multipart(body),
        _ => plain(&String::from_utf8_lossy(body)),
    }
}

fn multipart(body: &[u8]) -> Body {
    let content = MimeMessage::parse(body).content();
    match content.html() {
        Some(html) => Body::Markdown(Markdown::from_html(html)),
        None => plain(content.text()),
    }
}

fn plain(text: &str) -> Body {
    let text = text.replace("\r\n", "\n");
    let mut output = String::with_capacity(text.len());
    let mut blank = 0;

    for line in text.trim().lines() {
        let line = line.trim_end();
        if line.is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
        }
        output.push_str(line);
        output.push('\n');
    }

    Body::Plain(Plain::new(output.trim_end()))
}
