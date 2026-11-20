use std::sync::Arc;

use crate::inline::{Ink, Inline, Mark};
use crate::rendering::Rendering;
use pulldown_cmark::{
    Alignment, BlockQuoteKind, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd,
};
use rikka_ui::search::Found;
use rikka_ui::text::selection::Gather;
use rikka_ui::{Element, font};

pub(super) fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_HEADING_ATTRIBUTES
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS
        | Options::ENABLE_MATH
        | Options::ENABLE_GFM
        | Options::ENABLE_DEFINITION_LIST
        | Options::ENABLE_SUPERSCRIPT
        | Options::ENABLE_WIKILINKS
}

pub(crate) struct Builder<'a> {
    pub(super) rendering: &'a Rendering,
    gather: Option<Gather>,
    inline: Inline,
    pub(super) stack: Vec<Frame>,
}

pub(super) struct Frame {
    pub(super) kind: Kind,
    pub(super) blocks: Vec<Element>,
}

pub(super) enum Kind {
    Root,
    Paragraph,
    Heading(usize),
    Quote(Option<BlockQuoteKind>),
    List {
        start: Option<u64>,
    },
    Item {
        task: Option<bool>,
    },
    Code {
        text: String,
        speech: Option<String>,
    },
    Html {
        text: String,
    },
    Table {
        alignments: Vec<Alignment>,
        head: usize,
    },
    Head,
    Row,
    Cell {
        header: bool,
    },
    Footnote(String),
    Definitions,
    Term,
    Definition,
    Metadata,
}

impl<'a> Builder<'a> {
    pub(crate) fn new(rendering: &'a Rendering) -> Self {
        Self {
            rendering,
            gather: rendering.region.map(Gather::new),
            inline: Inline::default(),
            stack: vec![Frame {
                kind: Kind::Root,
                blocks: Vec::new(),
            }],
        }
    }

    pub(crate) fn gathering(mut self, gather: Gather) -> Self {
        self.gather = Some(gather);
        self
    }

    pub(crate) fn plain(self, source: &str) -> Arc<str> {
        let gather = self.gather.clone();
        let _ = self.build(source);
        gather
            .map(|gather| gather.text())
            .unwrap_or_else(|| Arc::from(""))
    }

    pub(super) fn gather(&self) -> Option<&Gather> {
        self.gather.as_ref()
    }

    pub(super) fn found(&self) -> Option<Found> {
        self.rendering.found.clone()
    }

    pub(super) fn unveils(&self) -> bool {
        self.rendering.unveiling
    }

    pub(super) fn opens(&self) {
        if let Some(gather) = &self.gather {
            gather.opens(self.lead());
        }
    }

    fn lead(&self) -> &'static str {
        match self.stack.last().map(|frame| &frame.kind) {
            Some(Kind::Cell { .. }) if !self.opens_row() => "\t",
            Some(Kind::Cell { .. }) if !self.opens_table() => "\n",
            Some(Kind::Item { .. }) if !self.opens_list() => "\n",
            Some(Kind::Term | Kind::Definition) => "\n",
            _ => "\n\n",
        }
    }

    fn starts(&self, depth: usize, kind: impl Fn(&Kind) -> bool) -> bool {
        self.stack
            .iter()
            .rev()
            .nth(depth)
            .is_some_and(|frame| kind(&frame.kind) && frame.blocks.is_empty())
    }

    fn opens_row(&self) -> bool {
        self.starts(1, |kind| matches!(kind, Kind::Row | Kind::Head))
    }

    fn opens_table(&self) -> bool {
        self.starts(2, |kind| matches!(kind, Kind::Table { .. }))
    }

    fn opens_list(&self) -> bool {
        self.starts(1, |kind| matches!(kind, Kind::List { .. }))
    }

    pub(crate) fn build(mut self, source: &str) -> Vec<Element> {
        for event in Parser::new_ext(source, options()) {
            self.event(event);
        }
        while self.stack.len() > 1 {
            self.close();
        }
        self.flush_inline();
        self.stack.pop().map(|root| root.blocks).unwrap_or_default()
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.text(&text),
            Event::Code(code) => self.inline.code(&code),
            Event::InlineMath(math) => self.inline.code(&format!("${math}$")),
            Event::DisplayMath(math) => {
                self.flush_inline();
                let block = self.code_block(&math, None);
                self.push(block);
            }
            Event::Html(html) => self.raw(&html),
            Event::InlineHtml(_) => {}
            Event::FootnoteReference(label) => {
                self.inline.open(Mark::Raised);
                self.inline.text(&format!("[{label}]"));
                self.inline.close(Mark::Raised);
            }
            Event::SoftBreak => self.inline.soft_break(),
            Event::HardBreak => self.inline.hard_break(),
            Event::Rule => {
                self.flush_inline();
                let rule = self.rule();
                self.push(rule);
            }
            Event::TaskListMarker(checked) => {
                if let Some(Frame {
                    kind: Kind::Item { task },
                    ..
                }) = self.stack.last_mut()
                {
                    *task = Some(checked);
                }
            }
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => self.open(Kind::Paragraph),
            Tag::Heading { level, .. } => self.open(Kind::Heading(depth_of(level))),
            Tag::BlockQuote(kind) => self.open(Kind::Quote(kind)),
            Tag::CodeBlock(kind) => self.open(Kind::Code {
                text: String::new(),
                speech: fenced(&kind),
            }),
            Tag::HtmlBlock => self.open(Kind::Html {
                text: String::new(),
            }),
            Tag::List(start) => self.open(Kind::List { start }),
            Tag::Item => self.open(Kind::Item { task: None }),
            Tag::FootnoteDefinition(label) => self.open(Kind::Footnote(label.into_string())),
            Tag::DefinitionList => self.open(Kind::Definitions),
            Tag::DefinitionListTitle => self.open(Kind::Term),
            Tag::DefinitionListDefinition => self.open(Kind::Definition),
            Tag::Table(alignments) => self.open(Kind::Table {
                alignments,
                head: 0,
            }),
            Tag::TableHead => self.open(Kind::Head),
            Tag::TableRow => self.open(Kind::Row),
            Tag::TableCell => {
                let header = self
                    .stack
                    .iter()
                    .any(|frame| matches!(frame.kind, Kind::Head));
                self.open(Kind::Cell { header });
            }
            Tag::Emphasis => self.inline.open(Mark::Italic),
            Tag::Strong => self.inline.open(Mark::Bold),
            Tag::Strikethrough => self.inline.open(Mark::Strike),
            Tag::Superscript | Tag::Subscript => self.inline.open(Mark::Raised),
            Tag::Link { dest_url, .. } => self.inline.open_link(dest_url.into_string()),
            Tag::Image { dest_url, .. } => {
                let ink = self.ink();
                self.inline.open_image(dest_url.into_string(), ink);
            }
            Tag::MetadataBlock(_) => self.open(Kind::Metadata),
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Emphasis => self.inline.close(Mark::Italic),
            TagEnd::Strong => self.inline.close(Mark::Bold),
            TagEnd::Strikethrough => self.inline.close(Mark::Strike),
            TagEnd::Superscript | TagEnd::Subscript => self.inline.close(Mark::Raised),
            TagEnd::Link => self.inline.close_link(),
            TagEnd::Image => self.inline.close_image(),
            _ => self.close(),
        }
    }

    fn text(&mut self, text: &str) {
        match self.stack.last_mut().map(|frame| &mut frame.kind) {
            Some(Kind::Code { text: body, .. } | Kind::Html { text: body }) => body.push_str(text),
            Some(Kind::Metadata) => {}
            _ => self.inline.text(text),
        }
    }

    fn raw(&mut self, html: &str) {
        if let Some(Kind::Html { text }) = self.stack.last_mut().map(|frame| &mut frame.kind) {
            text.push_str(html);
        }
    }

    fn open(&mut self, kind: Kind) {
        self.flush_inline();
        self.stack.push(Frame {
            kind,
            blocks: Vec::new(),
        });
    }

    fn close(&mut self) {
        self.flush_inline();
        self.keep_empty_cell();
        let Some(frame) = self.stack.pop() else {
            return;
        };
        for element in self.finish(frame) {
            self.push(element);
        }
    }

    fn keep_empty_cell(&self) {
        let empty = self.stack.last().is_some_and(|frame| {
            matches!(frame.kind, Kind::Cell { .. }) && frame.blocks.is_empty()
        });
        if let Some(gather) = self.gather.as_ref().filter(|_| empty) {
            gather.opens(self.lead());
            gather.skip();
        }
    }

    pub(super) fn push(&mut self, element: Element) {
        if let Some(frame) = self.stack.last_mut() {
            frame.blocks.push(element);
        }
    }

    fn ink(&self) -> Ink<'a> {
        Ink {
            style: &self.rendering.style,
            palette: self.rendering.palette,
            size: match self.stack.last().map(|frame| &frame.kind) {
                Some(Kind::Heading(level)) => self.rendering.style.heading_size(*level),
                _ => self.rendering.style.text_size,
            },
        }
    }

    fn flush_inline(&mut self) {
        if self.inline.is_empty() {
            return;
        }
        let ink = self.ink();
        let pieces = self.inline.take(ink);
        if pieces.is_empty() {
            return;
        }
        let element = match self.stack.last().map(|frame| &frame.kind) {
            Some(Kind::Heading(level)) => self.heading(pieces, *level),
            Some(Kind::Term | Kind::Cell { header: true }) => {
                self.prose(pieces, ink.size, font::SEMIBOLD, None)
            }
            _ => self.prose(pieces, ink.size, font::REGULAR, None),
        };
        self.push(element);
    }

    fn finish(&mut self, frame: Frame) -> Vec<Element> {
        match frame.kind {
            Kind::Root | Kind::Paragraph | Kind::Heading(_) | Kind::Term | Kind::Row => {
                frame.blocks
            }
            Kind::Metadata => Vec::new(),
            Kind::Quote(kind) => vec![self.quote(kind, frame.blocks)],
            Kind::List { .. } => vec![self.list(frame.blocks)],
            Kind::Item { task } => vec![self.item(task, frame.blocks)],
            Kind::Code { text, speech } => vec![self.code_block(&text, speech.as_deref())],
            Kind::Html { text } => vec![self.raw_block(&text)],
            Kind::Footnote(label) => vec![self.footnote(&label, frame.blocks)],
            Kind::Definitions => vec![self.definitions(frame.blocks)],
            Kind::Definition => vec![self.definition(frame.blocks)],
            Kind::Cell { .. } => vec![self.cell(frame.blocks)],
            Kind::Head => {
                let cells = frame.blocks;
                if let Some(Frame {
                    kind: Kind::Table { head, .. },
                    ..
                }) = self.stack.last_mut()
                {
                    *head = cells.len();
                }
                cells
            }
            Kind::Table { alignments, head } => vec![self.table(&alignments, head, frame.blocks)],
        }
    }
}
fn fenced(kind: &CodeBlockKind<'_>) -> Option<String> {
    match kind {
        CodeBlockKind::Fenced(info) => info
            .split(|letter: char| letter.is_whitespace() || letter == ',')
            .find(|word| !word.is_empty())
            .map(str::to_owned),
        CodeBlockKind::Indented => None,
    }
}
fn depth_of(level: HeadingLevel) -> usize {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}
