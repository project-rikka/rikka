use std::fmt::Write;

use rikka_html::{Document, Escaped};

use crate::message::Message;

const LINKS: [&str; 2] = ["https://", "http://"];
const LINK_ENDS: [char; 8] = ['.', ',', ';', ':', '!', '?', ')', '\''];

impl Message {
    #[must_use]
    pub fn readable(&self, picture: impl FnMut(&str) -> Option<String>) -> String {
        match (&self.html, &self.text) {
            (Some(html), _) => Document::parse(html).to_markdown_with_pictures(picture),
            (None, Some(text)) => Document::parse(&plain_as_html(text)).to_markdown(),
            (None, None) => String::new(),
        }
    }

    #[must_use]
    pub fn quotable(&self) -> String {
        match (&self.text, &self.html) {
            (Some(text), _) => text.trim_end().to_owned(),
            (None, Some(html)) => Document::parse(html).to_text(),
            (None, None) => String::new(),
        }
    }
}

fn plain_as_html(text: &str) -> String {
    let mut html = String::with_capacity(text.len() + text.len() / 4);
    let mut depth = 0;
    let mut open = false;
    for line in text.lines() {
        let (quotes, rest) = quoted(line);
        if quotes != depth {
            close(&mut html, &mut open);
            while depth < quotes {
                html.push_str("<blockquote>");
                depth += 1;
            }
            while depth > quotes {
                html.push_str("</blockquote>");
                depth -= 1;
            }
        }
        if rest.trim().is_empty() {
            close(&mut html, &mut open);
            continue;
        }
        match open {
            true => html.push_str("<br>"),
            false => {
                html.push_str("<p>");
                open = true;
            }
        }
        linked(&mut html, rest);
    }
    close(&mut html, &mut open);
    for _ in 0..depth {
        html.push_str("</blockquote>");
    }
    html
}

fn close(html: &mut String, open: &mut bool) {
    if std::mem::take(open) {
        html.push_str("</p>");
    }
}

fn quoted(line: &str) -> (usize, &str) {
    let mut rest = line;
    let mut quotes = 0;
    while let Some(inner) = rest.strip_prefix('>') {
        quotes += 1;
        rest = inner.strip_prefix(' ').unwrap_or(inner);
    }
    (quotes, rest)
}

fn linked(html: &mut String, line: &str) {
    let mut rest = line;
    loop {
        let start = LINKS.iter().filter_map(|scheme| rest.find(scheme)).min();
        let Some(start) = start else {
            break;
        };
        let (before, from) = rest.split_at(start);
        let length = from
            .find(|letter: char| letter.is_whitespace() || matches!(letter, '<' | '>' | '"'))
            .unwrap_or(from.len());
        let url = from[..length].trim_end_matches(LINK_ENDS);
        write!(
            html,
            "{}<a href=\"{}\">{}</a>",
            Escaped(before),
            Escaped(url),
            Escaped(url)
        )
        .expect("strings accept text");
        rest = &from[url.len()..];
    }
    write!(html, "{}", Escaped(rest)).expect("strings accept text");
}
