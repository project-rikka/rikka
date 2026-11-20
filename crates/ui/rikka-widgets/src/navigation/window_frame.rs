use rikka_ui::{
    Element, Layout, Length, PaintCtx, Quad, Styled, View, Widget, WindowChrome, motion,
};

use crate::blocks::container::container;
use crate::blocks::resize_edges::resize_edges;
use crate::navigation::title_bar::{CaptionBar, title_bar};

const TITLE_BAR_KEY: &str = "rikka:window-frame:title-bar";
const CONTENT_KEY: &str = "rikka:window-frame:content";
const RESIZE_EDGES_KEY: &str = "rikka:window-frame:resize-edges";

#[must_use]
pub struct WindowFrame {
    content: Option<Element>,
    caption: Option<CaptionBar>,
    chrome: WindowChrome,
}

pub fn window_frame(content: impl View) -> WindowFrame {
    WindowFrame {
        content: Some(content.build()),
        caption: None,
        chrome: WindowChrome::default(),
    }
}

impl WindowFrame {
    pub fn chrome(mut self, chrome: WindowChrome) -> Self {
        self.chrome = chrome;
        self
    }

    pub fn caption(mut self, bar: impl Into<CaptionBar>) -> Self {
        self.caption = Some(bar.into());
        self
    }

    fn floating(&self) -> bool {
        !self.chrome.maximized && !self.chrome.fullscreen
    }
}

impl Widget for WindowFrame {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let surface = ctx.theme().palette().background;
        let background = ctx.animate_curve("background", surface, motion::COLOR);
        let bounds = ctx.bounds();
        ctx.draw_quad(Quad::new(bounds).fill(background));
        ctx.glass(bounds);
        if background.a >= 1.0 {
            ctx.opaque(bounds);
        }
    }
}

impl std::fmt::Debug for WindowFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowFrame")
            .field("chrome", &self.chrome)
            .finish_non_exhaustive()
    }
}

impl View for WindowFrame {
    fn build(mut self) -> Element {
        let content = self.content.take().expect("content already taken");
        let caption = self.caption.take();
        let mut children = Vec::with_capacity(3);
        if !self.chrome.fullscreen {
            children.push(
                title_bar(&self.chrome, caption)
                    .build()
                    .keyed(TITLE_BAR_KEY),
            );
        }
        children.push(container(content).grow(1.0).build().keyed(CONTENT_KEY));
        let resizable = self.chrome.resizable && self.floating();
        children.push(resize_edges(resizable).build().keyed(RESIZE_EDGES_KEY));

        Element::with_children(self, children)
    }
}
