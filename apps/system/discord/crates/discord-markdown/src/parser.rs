use discord_core::{Block, Emoji, EmojiId, Inline, Mention, Rich, TimeStyle};

use crate::emoji::split_emoji;
use crate::rules;

#[derive(Clone, Copy)]
struct Scope {
    blocks: bool,
    quotes: bool,
    links: bool,
}

enum Node {
    Inline(Inline),
    Block(Block),
    Newline,
}

pub(crate) fn document(source: &str) -> Rich {
    let source = source.replace("\r\n", "\n").replace('\r', "\n");
    let scope = Scope {
        blocks: true,
        quotes: true,
        links: true,
    };
    Rich(blocks(nodes(&source, scope)))
}

pub(crate) fn line(source: &str) -> Rich {
    let source = source.replace("\r\n", "\n").replace('\r', "\n");
    let scope = Scope {
        blocks: false,
        quotes: false,
        links: true,
    };
    let content = inlines(nodes(&source, scope));
    if content.is_empty() {
        Rich::default()
    } else {
        Rich(vec![Block::Paragraph(content)])
    }
}

fn nodes(source: &str, scope: Scope) -> Vec<Node> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < source.len() {
        let rest = &source[at..];
        let line_start = at == 0 || source[..at].ends_with('\n');
        let (consumed, node) = next(rest, line_start, scope);
        out.push(node);
        at += consumed;
    }
    out
}

fn next(rest: &str, line_start: bool, scope: Scope) -> (usize, Node) {
    if scope.blocks
        && line_start
        && let Some(hit) = line_block(rest, scope)
    {
        return hit;
    }
    if let Some((consumed, language, text)) = rules::code_block(rest) {
        let node = if scope.blocks {
            Node::Block(Block::Code {
                language: language.map(str::to_owned),
                text: text.to_owned(),
            })
        } else {
            Node::Inline(Inline::Code(text.to_owned()))
        };
        return (consumed, node);
    }
    if rest.starts_with('\n') {
        return (1, Node::Newline);
    }
    if let Some(hit) = inline(rest, scope) {
        return (hit.0, Node::Inline(hit.1));
    }
    let consumed = rules::text(rest);
    (
        consumed,
        Node::Inline(Inline::Text(rest[..consumed].to_owned())),
    )
}

fn line_block(rest: &str, scope: Scope) -> Option<(usize, Node)> {
    if let Some((consumed, level, content)) = rules::heading(rest) {
        let block = Block::Heading {
            level,
            content: nested(content, scope),
        };
        return Some((consumed, Node::Block(block)));
    }
    if let Some((consumed, content)) = rules::subtext(rest) {
        return Some((
            consumed,
            Node::Block(Block::Subtext(nested(content, scope))),
        ));
    }
    if scope.quotes
        && let Some((consumed, content)) = rules::quote(rest)
    {
        let inner = Scope {
            quotes: false,
            ..scope
        };
        let block = Block::Quote(blocks(nodes(&content, inner)));
        return Some((consumed, Node::Block(block)));
    }
    if let Some((consumed, list)) = rules::list(rest) {
        let items = list
            .items
            .iter()
            .map(|item| blocks(nodes(item, scope)))
            .collect();
        let block = Block::List {
            start: list.start,
            items,
        };
        return Some((consumed, Node::Block(block)));
    }
    None
}

fn inline(rest: &str, scope: Scope) -> Option<(usize, Inline)> {
    if let Some((consumed, escaped)) = rules::escape(rest) {
        return Some((consumed, Inline::Text(escaped.to_owned())));
    }
    if scope.links {
        if let Some((consumed, target)) = rules::autolink(rest) {
            return Some((consumed, autolinked(target)));
        }
        if let Some((consumed, url)) = rules::url(rest) {
            return Some((consumed, linked(url)));
        }
        if let Some((consumed, label, href)) = rules::link(rest) {
            return Some((consumed, masked(&rest[..consumed], label, href, scope)));
        }
    }
    if let Some(hit) = emphasis(rest, scope) {
        return Some(hit);
    }
    if let Some((consumed, inner)) = rules::strike(rest) {
        return Some((consumed, Inline::Strike(nested(inner, scope))));
    }
    if let Some((consumed, inner)) = rules::spoiler(rest) {
        return Some((consumed, Inline::Spoiler(nested(inner, scope))));
    }
    if let Some((consumed, code)) = rules::inline_code(rest) {
        return Some((consumed, Inline::Code(code)));
    }
    reference(rest)
}

fn emphasis(rest: &str, scope: Scope) -> Option<(usize, Inline)> {
    let italic = rules::italic(rest).map(|(length, inner)| (length as f64 + 0.2, length, inner, 0));
    let bold = rules::bold(rest).map(|(length, inner)| (length as f64 + 0.1, length, inner, 1));
    let underline = rules::underline(rest).map(|(length, inner)| (length as f64, length, inner, 2));
    let best = [italic, bold, underline].into_iter().flatten().fold(
        None,
        |best: Option<(f64, usize, &str, u8)>, candidate| match best {
            Some(best) if best.0 > candidate.0 => Some(best),
            _ => Some(candidate),
        },
    )?;
    let content = nested(best.2, scope);
    let styled = match best.3 {
        0 => Inline::Italic(content),
        1 => Inline::Bold(content),
        _ => Inline::Underline(content),
    };
    Some((best.1, styled))
}

fn reference(rest: &str) -> Option<(usize, Inline)> {
    if !rest.starts_with(['<', '@']) {
        return None;
    }
    if let Some(rest_everyone) = rest.strip_prefix("@everyone") {
        return Some((
            rest.len() - rest_everyone.len(),
            Inline::Mention(Mention::Everyone),
        ));
    }
    if let Some(rest_here) = rest.strip_prefix("@here") {
        return Some((rest.len() - rest_here.len(), Inline::Mention(Mention::Here)));
    }
    if let Some((consumed, id)) = rules::role(rest) {
        return Some((consumed, Inline::Mention(Mention::Role(snowflake(id)))));
    }
    if let Some((consumed, id)) = rules::user(rest) {
        return Some((consumed, Inline::Mention(Mention::User(snowflake(id)))));
    }
    if let Some((consumed, id)) = rules::channel(rest) {
        return Some((consumed, Inline::Mention(Mention::Channel(snowflake(id)))));
    }
    if let Some((consumed, custom)) = rules::custom_emoji(rest) {
        let emoji = Emoji::custom(
            snowflake::<EmojiId>(custom.id),
            custom.name,
            custom.animated,
        );
        return Some((consumed, Inline::Emoji(emoji)));
    }
    if let Some((consumed, unix, style)) = rules::timestamp(rest) {
        return Some((
            consumed,
            Inline::Time {
                unix,
                style: time_style(style),
            },
        ));
    }
    if let Some((consumed, name)) = rules::command(rest) {
        return Some((consumed, Inline::Text(format!("/{name}"))));
    }
    None
}

fn time_style(style: Option<char>) -> TimeStyle {
    match style {
        Some('t') => TimeStyle::ShortTime,
        Some('T') => TimeStyle::LongTime,
        Some('d') => TimeStyle::ShortDate,
        Some('D') => TimeStyle::LongDate,
        Some('F') => TimeStyle::LongDateTime,
        Some('R') => TimeStyle::Relative,
        _ => TimeStyle::ShortDateTime,
    }
}

fn autolinked(target: &str) -> Inline {
    let scheme = target
        .split(':')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let allowed = matches!(
        scheme.as_str(),
        "http" | "https" | "discord" | "tel" | "sms" | "mailto"
    );
    let has_host = !matches!(scheme.as_str(), "http" | "https")
        || target
            .split_once("://")
            .is_some_and(|(_, after)| !after.is_empty() && !after.starts_with('/'));
    if allowed && has_host {
        linked(target)
    } else {
        Inline::Text(target.to_owned())
    }
}

fn linked(url: &str) -> Inline {
    Inline::Link {
        url: url.to_owned(),
        label: vec![Inline::Text(url.to_owned())],
    }
}

fn masked(whole: &str, label: &str, href: &str, scope: Scope) -> Inline {
    let href = href.trim();
    let safe = (href.starts_with("https://") || href.starts_with("http://"))
        && href.len() > href.find("://").unwrap_or(0) + 3
        && !label.trim().is_empty();
    if !safe {
        return Inline::Text(whole.to_owned());
    }
    let inner = Scope {
        links: false,
        ..scope
    };
    Inline::Link {
        url: href.to_owned(),
        label: nested(label, inner),
    }
}

fn nested(source: &str, scope: Scope) -> Vec<Inline> {
    let inner = Scope {
        blocks: false,
        quotes: false,
        ..scope
    };
    inlines(nodes(source, inner))
}

fn inlines(nodes: Vec<Node>) -> Vec<Inline> {
    let mut out = Vec::new();
    for node in nodes {
        match node {
            Node::Inline(inline) => out.push(inline),
            Node::Newline => out.push(Inline::Break),
            Node::Block(block) => out.push(Inline::Text(Rich(vec![block]).plain_text())),
        }
    }
    finish(out)
}

fn blocks(nodes: Vec<Node>) -> Vec<Block> {
    let mut out = Vec::new();
    let mut paragraph = Vec::new();
    let mut after_block = false;
    for node in nodes {
        match node {
            Node::Inline(inline) => {
                paragraph.push(inline);
                after_block = false;
            }
            Node::Newline if after_block => after_block = false,
            Node::Newline => paragraph.push(Inline::Break),
            Node::Block(block) => {
                close(&mut paragraph, &mut out);
                out.push(block);
                after_block = true;
            }
        }
    }
    close(&mut paragraph, &mut out);
    out
}

fn close(paragraph: &mut Vec<Inline>, out: &mut Vec<Block>) {
    while matches!(paragraph.last(), Some(Inline::Break)) {
        paragraph.pop();
    }
    if paragraph.is_empty() {
        return;
    }
    out.push(Block::Paragraph(finish(std::mem::take(paragraph))));
}

fn finish(inlines: Vec<Inline>) -> Vec<Inline> {
    let mut merged: Vec<Inline> = Vec::with_capacity(inlines.len());
    for inline in inlines {
        match (merged.last_mut(), inline) {
            (Some(Inline::Text(previous)), Inline::Text(text)) => previous.push_str(&text),
            (_, other) => merged.push(other),
        }
    }
    let mut out = Vec::with_capacity(merged.len());
    for inline in merged {
        match inline {
            Inline::Text(text) => split_emoji(&text, &mut out),
            other => out.push(other),
        }
    }
    out
}

fn snowflake<T: From<u64>>(text: &str) -> T {
    T::from(text.parse().unwrap_or(0))
}
