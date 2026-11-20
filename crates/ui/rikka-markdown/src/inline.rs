use std::ops::Range;

use rikka_text::{FontStyle, TextSpan};
use rikka_theme::Palette;
use rikka_ui::font;
use rikka_widgets::SpanFill;

use crate::style::MarkdownStyle;

const RAISED_SCALE: f32 = 0.75;

const CODE_SCALE: f32 = 0.92;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Ink<'a> {
    pub(crate) style: &'a MarkdownStyle,
    pub(crate) palette: Palette,
    pub(crate) size: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Face {
    bold: bool,
    italic: bool,
    strike: bool,
    code: bool,
    link: bool,
    raised: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mark {
    Bold,
    Italic,
    Strike,
    Link,
    Raised,
}

#[derive(Debug, Default)]
struct OpenMarks([u32; 5]);

impl OpenMarks {
    fn count(&mut self, mark: Mark) -> &mut u32 {
        &mut self.0[mark as usize]
    }

    fn has(&self, mark: Mark) -> bool {
        self.0[mark as usize] > 0
    }

    fn face(&self, code: bool) -> Face {
        Face {
            bold: self.has(Mark::Bold),
            italic: self.has(Mark::Italic),
            strike: self.has(Mark::Strike),
            code,
            link: self.has(Mark::Link),
            raised: self.has(Mark::Raised),
        }
    }
}

#[derive(Debug)]
pub(crate) enum Piece {
    Text(StyledText),
    Image { url: String, alt: String },
}

#[derive(Debug, Default)]
pub(crate) struct StyledText {
    pub(crate) text: String,
    pub(crate) spans: Vec<TextSpan>,
    pub(crate) fills: Vec<SpanFill>,
    pub(crate) links: Vec<Range<usize>>,
    pub(crate) targets: Vec<String>,
}

#[derive(Debug, Default)]
pub(crate) struct Inline {
    pieces: Vec<Piece>,
    pending: StyledText,
    faces: Vec<(Range<usize>, Face)>,
    open: OpenMarks,
    run: Option<(usize, Face)>,
    links: Vec<(usize, String)>,
    image: Option<String>,
}

impl Inline {
    pub(crate) fn is_empty(&self) -> bool {
        self.pieces.is_empty() && self.pending.text.trim().is_empty() && self.image.is_none()
    }

    pub(crate) fn text(&mut self, text: &str) {
        self.append(text, self.open.face(false));
    }

    pub(crate) fn code(&mut self, text: &str) {
        self.append(text, self.open.face(true));
    }

    pub(crate) fn soft_break(&mut self) {
        self.text(" ");
    }

    pub(crate) fn hard_break(&mut self) {
        self.text("\n");
    }

    pub(crate) fn open(&mut self, mark: Mark) {
        *self.open.count(mark) += 1;
    }

    pub(crate) fn close(&mut self, mark: Mark) {
        let count = self.open.count(mark);
        *count = count.saturating_sub(1);
    }

    pub(crate) fn open_link(&mut self, url: String) {
        self.open(Mark::Link);
        self.links.push((self.pending.text.len(), url));
    }

    pub(crate) fn close_link(&mut self) {
        self.close(Mark::Link);
        let Some((start, url)) = self.links.pop() else {
            return;
        };
        let end = self.pending.text.len();
        if end > start {
            self.pending.links.push(start..end);
            self.pending.targets.push(url);
        }
    }

    pub(crate) fn open_image(&mut self, url: String, ink: Ink<'_>) {
        self.flush(ink);
        self.image = Some(url);
    }

    pub(crate) fn close_image(&mut self) {
        let Some(url) = self.image.take() else {
            return;
        };
        self.run = None;
        self.faces.clear();
        let alt = std::mem::take(&mut self.pending).text;
        self.pieces.push(Piece::Image { url, alt });
    }

    pub(crate) fn take(&mut self, ink: Ink<'_>) -> Vec<Piece> {
        self.close_image();
        self.flush(ink);
        self.open = OpenMarks::default();
        self.links.clear();
        std::mem::take(&mut self.pieces)
    }

    fn append(&mut self, text: &str, face: Face) {
        if text.is_empty() {
            return;
        }
        match self.run {
            Some((_, open)) if open == face => {}
            _ => {
                self.close_run();
                self.run = Some((self.pending.text.len(), face));
            }
        }
        self.pending.text.push_str(text);
    }

    fn close_run(&mut self) {
        let Some((start, face)) = self.run.take() else {
            return;
        };
        let end = self.pending.text.len();
        if end > start && face != Face::default() {
            self.faces.push((start..end, face));
        }
    }

    fn flush(&mut self, ink: Ink<'_>) {
        self.close_run();
        self.style_spans(ink);
        let pending = std::mem::take(&mut self.pending);
        if !pending.text.is_empty() {
            self.pieces.push(Piece::Text(pending));
        }
    }

    fn style_spans(&mut self, ink: Ink<'_>) {
        for (range, face) in std::mem::take(&mut self.faces) {
            let mut span = TextSpan::new(range.clone());
            if face.bold {
                span = span.weight(font::SEMIBOLD);
            }
            if face.italic {
                span = span.style(FontStyle::Italic);
            }
            if face.strike {
                span = span.strikethrough(true);
            }
            if face.link {
                span = span.color(ink.palette.accent).underline(true);
            }
            if face.code {
                span = span
                    .family(ink.style.code_family.clone())
                    .font_size(ink.size * CODE_SCALE);
                self.pending
                    .fills
                    .push(SpanFill::new(range, ink.palette.surface_variant));
            } else if face.raised {
                span = span.font_size(ink.size * RAISED_SCALE);
            }
            self.pending.spans.push(span);
        }
    }
}
