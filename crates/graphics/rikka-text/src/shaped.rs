use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;

use rikka_text_layout::{
    Family, FontStyle, Paragraph, ShapedParagraph, StyleSpan, TextStyle, Weight,
};
use rustc_hash::FxHasher;

use crate::shaping::TextShape;
use crate::span::{TextSpan, valid_span_order};
use crate::text::Text;

#[derive(Debug)]
pub struct Shaped {
    text: Text,
    key: ShapeKey,
    lines: Box<[Line]>,
    bytes: usize,
}

#[derive(Debug)]
pub(crate) struct Line {
    pub(crate) range: Range<usize>,
    pub(crate) next: usize,
    pub(crate) shaped: Arc<ShapedLine>,
}

#[derive(Debug)]
pub(crate) struct ShapedLine {
    text: Box<str>,
    key: ShapeKey,
    pub(crate) paragraph: ShapedParagraph,
    pub(crate) bytes: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Face<'a> {
    family: &'a Family,
    weight: Weight,
    style: FontStyle,
    tab_width: u16,
}

impl<'a> Face<'a> {
    fn from_shape(shape: &TextShape<'a>) -> Self {
        Self {
            family: shape.family,
            weight: shape.weight,
            style: shape.style,
            tab_width: shape.tab_width,
        }
    }

    fn text_style(&self) -> TextStyle {
        TextStyle::new([self.family.clone()])
            .weight(self.weight)
            .style(self.style)
    }
}

#[derive(Debug)]
struct ShapeKey {
    family: Family,
    weight: Weight,
    style: FontStyle,
    tab_width: u16,
    spans: Box<[TextSpan]>,
}

impl ShapeKey {
    fn new(face: Face<'_>, spans: Box<[TextSpan]>) -> Self {
        Self {
            family: face.family.clone(),
            weight: face.weight,
            style: face.style,
            tab_width: face.tab_width,
            spans,
        }
    }

    fn is<'a>(&self, face: Face<'_>, spans: impl IntoIterator<Item = &'a TextSpan>) -> bool {
        let mut requested_spans = spans.into_iter();
        self.weight == face.weight
            && self.style == face.style
            && self.tab_width == face.tab_width
            && self.family == *face.family
            && self.spans.iter().all(|cached| {
                requested_spans
                    .next()
                    .is_some_and(|span| cached.shapes_like(span))
            })
            && requested_spans.next().is_none()
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LineRequest<'a> {
    pub(crate) text: &'a str,
    pub(crate) spans: &'a [TextSpan],
    pub(crate) face: Face<'a>,
}

impl LineRequest<'_> {
    pub(crate) fn digest(&self) -> u64 {
        let mut hasher = FxHasher::default();
        self.text.hash(&mut hasher);
        self.face.family.hash(&mut hasher);
        self.face.weight.hash(&mut hasher);
        self.face.style.hash(&mut hasher);
        self.face.tab_width.hash(&mut hasher);
        for span in self.spans {
            span.hash_shape(&mut hasher);
        }
        hasher.finish()
    }

    pub(crate) fn shape(&self) -> ShapedLine {
        let style = self.face.text_style();
        let spans: Vec<StyleSpan> = self.spans.iter().map(TextSpan::style_span).collect();
        let paragraph = Paragraph::new(self.text, &style)
            .spans(&spans)
            .tab_width(self.face.tab_width)
            .shape();
        ShapedLine {
            text: self.text.into(),
            key: ShapeKey::new(self.face, self.spans.into()),
            bytes: self.text.len() + paragraph.heap_bytes(),
            paragraph,
        }
    }
}

impl ShapedLine {
    pub(crate) fn is(&self, request: &LineRequest<'_>) -> bool {
        *self.text == *request.text && self.key.is(request.face, request.spans)
    }
}

impl Shaped {
    pub(crate) fn build(
        shape: &TextShape<'_>,
        mut shape_line: impl FnMut(LineRequest<'_>) -> Arc<ShapedLine>,
    ) -> Shaped {
        let text: &str = shape.text;
        let face = Face::from_shape(shape);
        let shaping: Vec<TextSpan> = shape
            .spans
            .iter()
            .filter(|span| span.shapes())
            .cloned()
            .collect();
        let order = valid_span_order(text, &shaping);
        let mut lines = Vec::new();
        let mut local = Vec::new();
        let mut first = 0;
        for (range, next) in paragraphs(text) {
            local.clear();
            while first < order.len() && shaping[order[first]].range.end <= range.start {
                first += 1;
            }
            for &index in &order[first..] {
                let span = &shaping[index];
                if span.range.start >= range.end {
                    break;
                }
                let start = span.range.start.max(range.start) - range.start;
                let end = span.range.end.min(range.end) - range.start;
                if start < end {
                    local.push(span.rebased(start..end));
                }
            }
            let request = LineRequest {
                text: &text[range.clone()],
                spans: &local,
                face,
            };
            let shaped = shape_line(request);
            lines.push(Line {
                range,
                next,
                shaped,
            });
        }
        let bytes =
            text.len() + lines.len() * size_of::<Line>() + shaping.len() * size_of::<TextSpan>();
        Shaped {
            text: shape.text.clone(),
            key: ShapeKey::new(face, shaping.into()),
            lines: lines.into(),
            bytes,
        }
    }

    pub(crate) fn is(&self, shape: &TextShape<'_>) -> bool {
        self.text == *shape.text
            && self.key.is(
                Face::from_shape(shape),
                shape.spans.iter().filter(|span| span.shapes()),
            )
    }

    #[must_use]
    pub fn text(&self) -> &Text {
        &self.text
    }

    #[must_use]
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub(crate) fn lines(&self) -> &[Line] {
        &self.lines
    }

    pub(crate) fn bytes(&self) -> usize {
        self.bytes
    }

    pub(crate) fn is_rtl(&self) -> bool {
        self.lines.iter().any(|line| {
            let paragraph = &line.shaped.paragraph;
            paragraph.is_rtl() || paragraph.is_bidi()
        })
    }
}

fn paragraphs(text: &str) -> Vec<(Range<usize>, usize)> {
    let bytes = text.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut at = 0;
    while at < bytes.len() {
        let ending = match bytes[at] {
            b'\r' if bytes.get(at + 1) == Some(&b'\n') => 2,
            b'\r' | b'\n' => 1,
            _ => 0,
        };
        if ending == 0 {
            at += 1;
            continue;
        }
        lines.push((start..at, at + ending));
        at += ending;
        start = at;
    }
    lines.push((start..text.len(), text.len()));
    lines
}
