use std::fmt;

use crate::selector::Selector;
use crate::token::{Tag, Token, tokens};

const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "keygen", "link", "meta", "param",
    "source", "track", "wbr", "basefont",
];

const SPECIAL: &[&str] = &[
    "address",
    "applet",
    "area",
    "article",
    "aside",
    "base",
    "blockquote",
    "body",
    "br",
    "button",
    "caption",
    "center",
    "col",
    "colgroup",
    "dd",
    "details",
    "dir",
    "div",
    "dl",
    "dt",
    "embed",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hgroup",
    "hr",
    "html",
    "iframe",
    "img",
    "input",
    "li",
    "link",
    "listing",
    "main",
    "marquee",
    "menu",
    "meta",
    "nav",
    "noembed",
    "noframes",
    "noscript",
    "object",
    "ol",
    "p",
    "param",
    "plaintext",
    "pre",
    "script",
    "section",
    "select",
    "source",
    "style",
    "summary",
    "table",
    "tbody",
    "td",
    "template",
    "textarea",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "track",
    "ul",
    "wbr",
    "xmp",
    "dialog",
    "search",
];

const CLOSES_PARAGRAPH: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "center",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "header",
    "hgroup",
    "main",
    "menu",
    "nav",
    "ol",
    "p",
    "section",
    "summary",
    "ul",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "pre",
    "listing",
    "form",
    "table",
    "hr",
    "li",
    "dd",
    "dt",
    "xmp",
];

const SCOPE: &[&str] = &[
    "applet", "caption", "table", "td", "th", "marquee", "object", "template",
];
const TABLE_SCOPE: &[&str] = &["table", "template"];
const TABLE_PARTS: &[&str] = &[
    "table", "tbody", "thead", "tfoot", "tr", "td", "th", "caption",
];
pub(crate) const ROW_GROUPS: &[&str] = &["tbody", "thead", "tfoot"];
pub(crate) const HEADINGS: &[&str] = &["h1", "h2", "h3", "h4", "h5", "h6"];
const FOREIGN: &[&str] = &["svg", "math"];
const ITEM_OPENERS: &[&str] = &["address", "div", "p"];

#[derive(Debug)]
pub struct Document {
    nodes: Vec<Node>,
}

#[derive(Debug)]
struct Node {
    parent: usize,
    end: usize,
    content: Content,
}

#[derive(Debug)]
enum Content {
    Root,
    Element {
        name: Box<str>,
        attributes: Vec<(Box<str>, Box<str>)>,
    },
    Text(Box<str>),
}

#[derive(Clone, Copy)]
pub struct Element<'a> {
    document: &'a Document,
    index: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Child<'a> {
    Element(Element<'a>),
    Text(&'a str),
}

impl Document {
    #[must_use]
    pub fn parse(html: &str) -> Document {
        let mut builder = Builder {
            nodes: vec![Node {
                parent: 0,
                end: 0,
                content: Content::Root,
            }],
            open: vec![0],
        };
        for token in tokens(html) {
            match token {
                Token::Text(text) => builder.text(&text),
                Token::Open(tag) => builder.start(tag),
                Token::Close(name) => builder.end(&name),
            }
        }
        builder.pop_to(0);
        Document {
            nodes: builder.nodes,
        }
    }

    pub fn select<'a>(&'a self, selector: &'a Selector) -> impl Iterator<Item = Element<'a>> {
        self.selected(1..self.nodes.len(), selector)
    }

    pub fn text(&self) -> impl Iterator<Item = &str> {
        self.texts(0)
    }

    pub(crate) fn children(&self) -> impl Iterator<Item = Child<'_>> {
        self.children_of(0)
    }

    fn children_of(&self, index: usize) -> impl Iterator<Item = Child<'_>> {
        let end = self.nodes[index].end;
        let mut at = index + 1;
        std::iter::from_fn(move || {
            if at >= end {
                return None;
            }
            let child = at;
            at = self.nodes[child].end;
            Some(match &self.nodes[child].content {
                Content::Text(text) => Child::Text(text),
                Content::Root | Content::Element { .. } => Child::Element(Element {
                    document: self,
                    index: child,
                }),
            })
        })
    }

    fn selected<'a>(
        &'a self,
        range: std::ops::Range<usize>,
        selector: &'a Selector,
    ) -> impl Iterator<Item = Element<'a>> {
        range
            .filter_map(move |index| self.element(index))
            .filter(move |element| selector.matches(*element))
    }

    fn texts(&self, index: usize) -> impl Iterator<Item = &str> {
        self.nodes[index + 1..self.nodes[index].end]
            .iter()
            .filter_map(|node| match &node.content {
                Content::Text(text) => Some(text.as_ref()),
                Content::Root | Content::Element { .. } => None,
            })
    }

    fn element(&self, index: usize) -> Option<Element<'_>> {
        matches!(self.nodes[index].content, Content::Element { .. }).then_some(Element {
            document: self,
            index,
        })
    }
}

impl<'a> Element<'a> {
    #[must_use]
    pub fn name(self) -> &'a str {
        match &self.document.nodes[self.index].content {
            Content::Element { name, .. } => name,
            Content::Root | Content::Text(_) => "",
        }
    }

    #[must_use]
    pub fn attribute(self, name: &str) -> Option<&'a str> {
        match &self.document.nodes[self.index].content {
            Content::Element { attributes, .. } => attributes
                .iter()
                .find(|(known, _)| **known == *name)
                .map(|(_, value)| value.as_ref()),
            Content::Root | Content::Text(_) => None,
        }
    }

    #[must_use]
    pub fn has_class(self, class: &str) -> bool {
        self.attribute("class")
            .is_some_and(|classes| classes.split_ascii_whitespace().any(|known| known == class))
    }

    #[must_use]
    pub fn parent(self) -> Option<Element<'a>> {
        self.document
            .element(self.document.nodes[self.index].parent)
    }

    pub fn text(self) -> impl Iterator<Item = &'a str> {
        self.document.texts(self.index)
    }

    pub(crate) fn children(self) -> impl Iterator<Item = Child<'a>> {
        self.document.children_of(self.index)
    }

    pub(crate) fn descendants(self) -> impl Iterator<Item = Element<'a>> {
        let document = self.document;
        (self.index + 1..document.nodes[self.index].end).filter_map(|index| document.element(index))
    }

    pub fn select(self, selector: &'a Selector) -> impl Iterator<Item = Element<'a>> {
        self.document.selected(
            self.index + 1..self.document.nodes[self.index].end,
            selector,
        )
    }
}

impl fmt::Debug for Element<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Element")
            .field("name", &self.name())
            .field("index", &self.index)
            .finish()
    }
}

struct Builder {
    nodes: Vec<Node>,
    open: Vec<usize>,
}

impl Builder {
    fn name(&self, depth: usize) -> &str {
        match &self.nodes[self.open[depth]].content {
            Content::Element { name, .. } => name,
            Content::Root | Content::Text(_) => "",
        }
    }

    fn top(&self) -> &str {
        self.name(self.open.len() - 1)
    }

    fn text(&mut self, text: &str) {
        self.leaf(Content::Text(text.into()));
    }

    fn leaf(&mut self, content: Content) -> usize {
        let index = self.nodes.len();
        self.nodes.push(Node {
            parent: self.open[self.open.len() - 1],
            end: index + 1,
            content,
        });
        index
    }

    fn start(&mut self, tag: Tag<'_>) {
        let foreign = (1..self.open.len()).any(|depth| FOREIGN.contains(&self.name(depth)));
        if !foreign {
            self.imply_end(tag.name());
        }
        let childless = VOID.contains(&tag.name())
            || (tag.is_self_closing() && (foreign || FOREIGN.contains(&tag.name())));
        let (name, attributes) = tag.into_parts();
        let index = self.leaf(Content::Element {
            name: name.into(),
            attributes: attributes
                .into_iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect(),
        });
        if !childless {
            self.open.push(index);
        }
    }

    fn end(&mut self, name: &str) {
        let special = SPECIAL.contains(&name);
        for depth in (1..self.open.len()).rev() {
            let open = self.name(depth);
            if open == name {
                self.pop_to(depth);
                return;
            }
            let blocked = match (special, TABLE_PARTS.contains(&name)) {
                (true, true) => TABLE_SCOPE.contains(&open),
                (true, false) => SCOPE.contains(&open),
                (false, _) => SPECIAL.contains(&open),
            };
            if blocked {
                return;
            }
        }
    }

    fn imply_end(&mut self, name: &str) {
        match name {
            "li" => self.close_item(&["li"]),
            "dd" | "dt" => self.close_item(&["dd", "dt"]),
            "option" => self.pop_top(&["option"]),
            "optgroup" => {
                self.pop_top(&["option"]);
                self.pop_top(&["optgroup"]);
            }
            "tr" => self.clear_to(ROW_GROUPS),
            "td" | "th" => self.clear_to(&["tr"]),
            "tbody" | "thead" | "tfoot" => self.clear_to(&[]),
            _ if HEADINGS.contains(&name) => self.pop_top(HEADINGS),
            _ => {}
        }
        if CLOSES_PARAGRAPH.contains(&name) {
            self.close_paragraph();
        }
    }

    fn pop_top(&mut self, names: &[&str]) {
        if names.contains(&self.top()) {
            self.pop_to(self.open.len() - 1);
        }
    }

    fn close_item(&mut self, names: &[&str]) {
        for depth in (1..self.open.len()).rev() {
            let open = self.name(depth);
            if names.contains(&open) {
                self.pop_to(depth);
                return;
            }
            if SPECIAL.contains(&open) && !ITEM_OPENERS.contains(&open) {
                return;
            }
        }
    }

    fn close_paragraph(&mut self) {
        for depth in (1..self.open.len()).rev() {
            let open = self.name(depth);
            if open == "p" {
                self.pop_to(depth);
                return;
            }
            if open == "button" || SCOPE.contains(&open) {
                return;
            }
        }
    }

    fn clear_to(&mut self, names: &[&str]) {
        let Some(table) = (1..self.open.len())
            .rev()
            .find(|depth| TABLE_SCOPE.contains(&self.name(*depth)))
        else {
            return;
        };
        let kept = (table + 1..self.open.len())
            .rev()
            .find(|depth| names.contains(&self.name(*depth)))
            .unwrap_or(table);
        self.pop_to(kept + 1);
    }

    fn pop_to(&mut self, depth: usize) {
        let end = self.nodes.len();
        for index in self.open.drain(depth..) {
            self.nodes[index].end = end;
        }
    }
}
