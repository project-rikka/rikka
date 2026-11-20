use std::rc::Rc;

use crate::document::{Child, Document, Element, HEADINGS};

use super::block::{Block, LAST_NUMBER, List};
use super::phrasing::{Gathering, Style};
use super::{css, table};

const DEEPEST_ELEMENT: usize = 256;
const DEEPEST_NESTING: usize = 16;
const LINK_SCHEMES: [&str; 3] = ["http://", "https://", "mailto:"];

const SKIPPED: &[&str] = &[
    "applet", "area", "audio", "base", "canvas", "datalist", "embed", "frame", "frameset", "head",
    "iframe", "input", "link", "map", "math", "meta", "noembed", "noframes", "noscript", "object",
    "optgroup", "option", "param", "script", "select", "source", "style", "svg", "template",
    "textarea", "title", "track", "video",
];

const BLOCKS: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "body",
    "caption",
    "center",
    "dd",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "header",
    "hgroup",
    "html",
    "legend",
    "li",
    "main",
    "menu",
    "nav",
    "ol",
    "p",
    "search",
    "section",
    "summary",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "tr",
    "ul",
];

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Depth {
    elements: usize,
    nesting: usize,
}

impl Depth {
    pub(super) fn deeper(self, elements: usize) -> Depth {
        Depth {
            elements: self.elements + elements,
            ..self
        }
    }

    fn nested(self) -> Depth {
        Depth {
            nesting: self.nesting + 1,
            ..self
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct Flow {
    blocks: Vec<Block>,
    gathering: Gathering,
}

impl Flow {
    pub(super) fn paragraph_break(&mut self) {
        if let Some(phrasing) = self.gathering.finish() {
            self.blocks.push(Block::Paragraph(phrasing));
        }
    }

    pub(super) fn push(&mut self, block: Block) {
        self.paragraph_break();
        if matches!(block, Block::Rule) && matches!(self.blocks.last(), None | Some(Block::Rule)) {
            return;
        }
        self.blocks.push(block);
    }

    pub(super) fn finish(mut self) -> Vec<Block> {
        self.paragraph_break();
        if matches!(self.blocks.last(), Some(Block::Rule)) {
            self.blocks.pop();
        }
        self.blocks
    }
}

pub(super) fn blocks(document: &Document) -> Vec<Block> {
    let mut flow = Flow::default();
    for child in document.children() {
        node(child, &Style::default(), &mut flow, Depth::default());
    }
    flow.finish()
}

pub(super) fn node(child: Child<'_>, style: &Style, flow: &mut Flow, depth: Depth) {
    match child {
        Child::Text(text) => flow.gathering.text(text, style),
        Child::Element(element) => self::element(element, style, flow, depth),
    }
}

pub(super) fn contents(element: Element<'_>, style: &Style, depth: Depth) -> Vec<Block> {
    let mut flow = Flow::default();
    self::element(element, style, &mut flow, depth);
    flow.finish()
}

fn element(element: Element<'_>, style: &Style, flow: &mut Flow, depth: Depth) {
    let name = element.name();
    if SKIPPED.contains(&name) {
        return;
    }
    let Some(style) = css::styled(element, marked(element, style)) else {
        return;
    };
    let style = &style;
    if depth.elements >= DEEPEST_ELEMENT {
        for child in visible(element) {
            match child {
                Child::Text(text) => flow.gathering.text(text, style),
                Child::Element(inner) if inner.name() == "br" => flow.gathering.line_break(),
                Child::Element(_) => {}
            }
        }
        return;
    }
    let depth = depth.deeper(1);
    match name {
        "br" => flow.gathering.line_break(),
        "img" => {
            if style.link.is_some()
                && let Some(alt) = element.attribute("alt")
            {
                flow.gathering.text(alt, style);
            }
        }
        "hr" => flow.push(Block::Rule),
        "pre" | "xmp" | "listing" | "plaintext" => {
            if let Some(code) = preformatted(element) {
                flow.push(Block::Code(code));
            }
        }
        _ if HEADINGS.contains(&name) => {
            let level = usize::from(name.as_bytes()[1] - b'0');
            let mut inner = Flow::default();
            children(element, style, &mut inner, depth);
            for block in inner.finish() {
                flow.push(match block {
                    Block::Paragraph(phrasing) => Block::Heading(level, phrasing),
                    other => other,
                });
            }
        }
        "blockquote" if depth.nesting < DEEPEST_NESTING => {
            let mut inner = Flow::default();
            children(element, style, &mut inner, depth.nested());
            let blocks = inner.finish();
            if !blocks.is_empty() {
                flow.push(Block::Quote(blocks));
            }
        }
        "ul" | "ol" | "menu" | "dir" if depth.nesting < DEEPEST_NESTING => {
            list(element, style, flow, depth.nested());
        }
        "table" => table::table(element, style, flow, depth),
        _ if BLOCKS.contains(&name) => {
            flow.paragraph_break();
            children(element, style, flow, depth);
            flow.paragraph_break();
        }
        _ => children(element, style, flow, depth),
    }
}

fn children(element: Element<'_>, style: &Style, flow: &mut Flow, depth: Depth) {
    for child in element.children() {
        node(child, style, flow, depth);
    }
}

fn marked(element: Element<'_>, style: &Style) -> Style {
    let mut marked = style.clone();
    match element.name() {
        "b" | "strong" | "dt" => marked.strong = true,
        "i" | "em" | "cite" | "dfn" | "var" => marked.emphasis = true,
        "s" | "strike" | "del" => marked.strike = true,
        "code" | "kbd" | "samp" | "tt" => marked.code = true,
        "a" => {
            if let Some(target) = element.attribute("href").and_then(link_target) {
                marked.link = Some(target);
            }
        }
        _ => {}
    }
    marked
}

fn link_target(href: &str) -> Option<Rc<str>> {
    let target: String = href
        .trim_matches(|character: char| character.is_ascii_whitespace())
        .chars()
        .filter(|character| !matches!(character, '\t' | '\n' | '\r'))
        .collect();
    let lowered = target.to_ascii_lowercase();
    LINK_SCHEMES
        .iter()
        .any(|scheme| lowered.len() > scheme.len() && lowered.starts_with(scheme))
        .then(|| Rc::from(target))
}

fn list(element: Element<'_>, style: &Style, flow: &mut Flow, depth: Depth) {
    let start = match element.name() {
        "ol" => Some(
            element
                .attribute("start")
                .and_then(|start| start.trim().parse::<u64>().ok())
                .unwrap_or(1)
                .min(LAST_NUMBER),
        ),
        _ => None,
    };
    let mut items = Vec::new();
    let mut item: Option<Flow> = None;
    for child in element.children() {
        match child {
            Child::Element(entry) if entry.name() == "li" => {
                if let Some(done) = item.take() {
                    items.push(done.finish());
                }
                let mut fresh = Flow::default();
                self::element(entry, style, &mut fresh, depth);
                item = Some(fresh);
            }
            other => node(other, style, item.get_or_insert_with(Flow::default), depth),
        }
    }
    if let Some(done) = item {
        items.push(done.finish());
    }
    items.retain(|blocks| !blocks.is_empty());
    if !items.is_empty() {
        flow.push(Block::List(List { start, items }));
    }
}

fn preformatted(element: Element<'_>) -> Option<String> {
    let mut text = String::new();
    for child in visible(element) {
        match child {
            Child::Text(part) => text.push_str(&part.replace("\r\n", "\n").replace('\r', "\n")),
            Child::Element(inner) if inner.name() == "br" => text.push('\n'),
            Child::Element(_) => {}
        }
    }
    let text = text.strip_prefix('\n').unwrap_or(&text).trim_end();
    (!text.trim().is_empty()).then(|| text.to_owned())
}

fn visible<'a>(element: Element<'a>) -> impl Iterator<Item = Child<'a>> {
    let mut stack = vec![element.children()];
    std::iter::from_fn(move || {
        loop {
            let Some(child) = stack.last_mut()?.next() else {
                stack.pop();
                continue;
            };
            if let Child::Element(inner) = child {
                if SKIPPED.contains(&inner.name()) || css::is_hidden(inner) {
                    continue;
                }
                stack.push(inner.children());
            }
            return Some(child);
        }
    })
}
