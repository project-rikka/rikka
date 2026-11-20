use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use rikka_ui::{Element, Layout, NodeHandle, Style, Styled, View, Watchers, Widget};

use crate::Markdown;

#[derive(Clone, Debug, Default)]
pub struct MarkdownStream(Rc<StreamBuffer>);

#[derive(Debug, Default)]
struct StreamBuffer {
    piece: Cell<Option<usize>>,
    text: RefCell<String>,
    watchers: Watchers,
}

impl MarkdownStream {
    pub fn follow(&self, piece: Option<usize>, text: &str) {
        let flowing = &self.0;
        if flowing.piece.get() == piece && *flowing.text.borrow() == text {
            return;
        }
        flowing.piece.set(piece);
        let mut buffer = flowing.text.borrow_mut();
        buffer.clear();
        buffer.push_str(text);
        drop(buffer);
        self.notify_changed();
    }

    #[must_use]
    pub fn grow(&self, piece: usize, text: &str) -> bool {
        let flowing = &self.0;
        if flowing.piece.get() != Some(piece) {
            return false;
        }
        let mut buffer = flowing.text.borrow_mut();
        if *buffer == text {
            return true;
        }
        if buffer.is_empty() || !text.starts_with(buffer.as_str()) {
            return false;
        }
        let more = &text[buffer.len()..];
        buffer.push_str(more);
        drop(buffer);
        self.notify_changed();
        true
    }

    #[must_use]
    pub fn carries(&self, piece: usize) -> bool {
        self.0.piece.get() == Some(piece)
    }

    fn notify_changed(&self) {
        self.0.watchers.rebuild();
    }

    fn text(&self) -> Arc<str> {
        Arc::from(self.0.text.borrow().as_str())
    }
}

pub(crate) struct StreamSource {
    stream: MarkdownStream,
    template: Markdown,
}

impl StreamSource {
    pub(crate) fn new(stream: MarkdownStream, template: Markdown) -> StreamSource {
        StreamSource { stream, template }
    }
}

impl Widget for StreamSource {
    type State = ();

    fn layout(&self) -> Layout {
        let mut layout = Layout::column();
        self.template.layout.apply(&mut layout);
        layout
    }

    fn build(&self, _state: &mut ()) -> Vec<Element> {
        self.stream.0.watchers.watch(NodeHandle::current());
        let mut markdown = self.template.clone();
        markdown.source = self.stream.text();
        markdown.layout = Style::default();
        vec![markdown.fill_width().build()]
    }

    fn debug_name(&self) -> &'static str {
        "StreamedMarkdown"
    }
}
