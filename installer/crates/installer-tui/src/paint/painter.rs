use ratatui::{
    Frame,
    layout::{Position, Rect},
    widgets::Widget,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{glyphs::Glyphs, theme::Theme};

pub(crate) struct Painter<'a, 'b> {
    frame: &'a mut Frame<'b>,
    theme: Theme,
    glyphs: Glyphs,
}

impl<'a, 'b> Painter<'a, 'b> {
    pub(crate) fn new(frame: &'a mut Frame<'b>, theme: Theme, glyphs: Glyphs) -> Self {
        Self {
            frame,
            theme,
            glyphs,
        }
    }

    pub(crate) fn theme(&self) -> Theme {
        self.theme
    }

    pub(crate) fn glyphs(&self) -> Glyphs {
        self.glyphs
    }

    pub(crate) fn area(&self) -> Rect {
        self.frame.area()
    }

    pub(crate) fn render(&mut self, widget: impl Widget, area: Rect) {
        self.frame.render_widget(widget, area);
    }

    pub(crate) fn caret(&mut self, position: Position) {
        self.frame.set_cursor_position(position);
    }

    pub(crate) fn fit(&self, text: &str, width: usize) -> String {
        if text.width() <= width {
            return text.to_owned();
        }
        let mark = self.glyphs.ellipsis;
        if width < mark.width() {
            return take(text, width);
        }
        let mut out = take(text, width - mark.width());
        out.push_str(mark);
        out
    }

    pub(crate) fn pad(&self, text: &str, width: usize) -> String {
        let mut out = self.fit(text, width);
        let used = out.width();
        out.reserve(width.saturating_sub(used));
        for _ in used..width {
            out.push(' ');
        }
        out
    }
}

fn take(text: &str, width: usize) -> String {
    let mut out = String::with_capacity(width);
    let mut used = 0;
    for ch in text.chars() {
        let next = used + ch.width().unwrap_or(0);
        if next > width {
            break;
        }
        out.push(ch);
        used = next;
    }
    out
}
