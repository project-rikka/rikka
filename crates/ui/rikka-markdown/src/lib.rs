mod blocks;
mod build;
mod document;
mod inline;
mod rendering;
mod stream;
mod style;

use rikka_ui::{Element, Handler, IntoHandler, Length, Style, Styled, View, hash_of, motion};
use std::hash::Hash;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
pub use stream::MarkdownStream;

use crate::build::Builder;
use crate::document::{Block, Document};
use crate::rendering::Rendering;
use crate::stream::StreamSource;
use rikka_ui::search::Found;
use rikka_ui::text::selection::{Gather, Region, SharedWhole};
use rikka_widgets::{column_of, container, deferred, lazy};

pub(crate) type LinkHandler = Handler<(String,)>;

const BLOCK_AT_A_TIME: usize = 4 * 1024;

const RISE: f32 = 6.0;

#[must_use]
pub fn markdown_text(source: &str) -> Arc<str> {
    Arc::clone(Document::parse_cached(&Arc::from(source)).text())
}

#[must_use]
#[derive(Clone)]
pub struct Markdown {
    source: Arc<str>,
    stream: Option<MarkdownStream>,
    rendering: Rendering,
    layout: Style,
}

pub fn markdown(source: impl Into<Arc<str>>) -> Markdown {
    Markdown {
        source: source.into(),
        stream: None,
        rendering: Rendering::from_theme(&rikka_ui::theme()),
        layout: Style::default(),
    }
}

pub fn streamed_markdown(stream: &MarkdownStream) -> Markdown {
    let mut markdown = markdown("");
    markdown.stream = Some(stream.clone());
    markdown.rendering.unveiling = true;
    markdown
}

impl Markdown {
    pub fn on_link<Marker>(mut self, on_link: impl IntoHandler<(String,), (), Marker>) -> Self {
        self.rendering.on_link = Some(on_link.into_handler());
        self
    }

    pub fn base(mut self, base: impl Into<PathBuf>) -> Self {
        self.rendering.base = Some(base.into());
        self
    }

    pub fn selectable(mut self, key: impl Hash) -> Self {
        self.rendering.region = Some(Region::new(key));
        self
    }

    pub fn found(mut self, found: Found) -> Self {
        self.rendering.found = Some(found);
        self
    }
}

impl std::fmt::Debug for Markdown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Markdown")
            .field("bytes", &self.source.len())
            .finish_non_exhaustive()
    }
}

impl View for Markdown {
    fn build(mut self) -> Element {
        if let Some(stream) = self.stream.take() {
            return Element::new(StreamSource::new(stream, self));
        }
        let gap = self.rendering.style.block_gap;
        let layout = self.layout;
        let blocks = match (
            self.rendering.unveiling,
            self.source.len() > BLOCK_AT_A_TIME,
        ) {
            (true, _) => self.unveiled_blocks(),
            (false, true) => self.deferred_blocks(),
            (false, false) => self.build_all(),
        };
        column_of(blocks).gap(gap).styled(layout).build()
    }
}

fn with_entrance(block: Element) -> Element {
    container(block.transition((motion::fade() + motion::offset(0.0, RISE)).enter(motion::ARRIVAL)))
        .width(Length::FILL)
        .build()
}

impl Markdown {
    fn build_all(self) -> Vec<Element> {
        Builder::new(&self.rendering).build(&self.source)
    }

    fn deferred_blocks(self) -> Vec<Element> {
        let line = self.rendering.style.line_height + self.rendering.style.block_gap;
        self.each_block(|digest, block, state| {
            deferred(digest, block.lines * line, move || state.build(&block))
                .width(Length::FILL)
                .build()
        })
    }

    fn unveiled_blocks(self) -> Vec<Element> {
        self.each_block(|digest, block, state| {
            with_entrance(
                lazy(digest, move || state.build(&block))
                    .width(Length::FILL)
                    .build(),
            )
        })
    }

    fn each_block(self, place: impl Fn(u64, Block, Rc<DocumentState>) -> Element) -> Vec<Element> {
        let parsed = Document::parse_cached(&self.source);
        let searched = self
            .rendering
            .found
            .as_ref()
            .map(|found| (found.query().stamp(), found.is_empty()));
        let whole = self.rendering.region.map(|region| {
            let whole = SharedWhole::for_region(region);
            whole.set(parsed.text());
            whole
        });
        let gathers = whole.is_some();
        let state = Rc::new(DocumentState {
            parsed: Arc::clone(&parsed),
            rendering: self.rendering,
            whole,
        });
        parsed
            .blocks()
            .iter()
            .map(|block| {
                let digest = hash_of((digest(&parsed, block, gathers), searched));
                place(digest, block.clone(), Rc::clone(&state))
            })
            .collect()
    }
}

type Digest = (u64, u64, Option<usize>);

fn digest(parsed: &Document, block: &Block, gathers: bool) -> Digest {
    let gathering = gathers.then_some(block.text.start);
    (block.key, parsed.refs_key(), gathering)
}

struct DocumentState {
    parsed: Arc<Document>,
    rendering: Rendering,
    whole: Option<Rc<SharedWhole>>,
}

impl DocumentState {
    fn build(&self, block: &Block) -> Element {
        let source = self.parsed.block_source(block);
        let mut builder = Builder::new(&self.rendering);
        if let Some(whole) = &self.whole {
            builder = builder.gathering(Gather::shared(whole, block.text.start));
        }
        let mut blocks = builder.build(&source);
        match blocks.len() {
            1 => blocks.remove(0),
            _ => column_of(blocks)
                .gap(self.rendering.style.block_gap)
                .width(rikka_ui::Length::FILL)
                .build(),
        }
    }
}

impl Styled for Markdown {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.layout
    }
}
