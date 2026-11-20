use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::{Arc, LazyLock};

use rikka_text_layout::{Family, FontStyle, StyleSpan, Weight};
use rikka_types::Color;

pub(crate) fn no_spans() -> Arc<[TextSpan]> {
    static EMPTY: LazyLock<Arc<[TextSpan]>> = LazyLock::new(|| Arc::from(Vec::new()));
    EMPTY.clone()
}

#[derive(Clone, Debug, Default, PartialEq)]
#[must_use]
pub struct TextSpan {
    pub range: Range<usize>,
    pub color: Option<Color>,
    pub family: Option<Family>,
    pub weight: Option<Weight>,
    pub style: Option<FontStyle>,
    pub font_size: Option<f32>,
    pub underline: bool,
    pub strikethrough: bool,
}

impl TextSpan {
    pub fn new(range: Range<usize>) -> Self {
        Self {
            range,
            ..Self::default()
        }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn family(mut self, family: impl Into<Family>) -> Self {
        self.family = Some(family.into());
        self
    }

    pub fn weight(mut self, weight: Weight) -> Self {
        self.weight = Some(weight);
        self
    }

    pub fn style(mut self, style: FontStyle) -> Self {
        self.style = Some(style);
        self
    }

    pub fn font_size(mut self, font_size: f32) -> Self {
        self.font_size = Some(font_size);
        self
    }

    pub fn underline(mut self, underline: bool) -> Self {
        self.underline = underline;
        self
    }

    pub fn strikethrough(mut self, strikethrough: bool) -> Self {
        self.strikethrough = strikethrough;
        self
    }

    pub(crate) fn shapes(&self) -> bool {
        self.family.is_some()
            || self.weight.is_some()
            || self.style.is_some()
            || self.font_size.is_some()
            || self.underline
            || self.strikethrough
    }

    pub(crate) fn shapes_like(&self, other: &TextSpan) -> bool {
        self.range == other.range
            && self.family == other.family
            && self.weight == other.weight
            && self.style == other.style
            && self.font_size.map(f32::to_bits) == other.font_size.map(f32::to_bits)
            && self.underline == other.underline
            && self.strikethrough == other.strikethrough
    }

    pub(crate) fn hash_shape(&self, state: &mut impl Hasher) {
        self.range.hash(state);
        self.family.hash(state);
        self.weight.hash(state);
        self.style.hash(state);
        self.font_size.map(f32::to_bits).hash(state);
        self.underline.hash(state);
        self.strikethrough.hash(state);
    }

    pub(crate) fn style_span(&self) -> StyleSpan {
        let mut span = StyleSpan::new(self.range.clone())
            .underline(self.underline)
            .strikethrough(self.strikethrough);
        if let Some(family) = &self.family {
            span = span.family(family.clone());
        }
        if let Some(weight) = self.weight {
            span = span.weight(weight);
        }
        if let Some(style) = self.style {
            span = span.style(style);
        }
        if let Some(font_size) = self.font_size {
            span = span.font_size(font_size.max(1.0));
        }
        span
    }

    pub(crate) fn rebased(&self, range: Range<usize>) -> TextSpan {
        TextSpan {
            range,
            color: None,
            ..self.clone()
        }
    }
}

pub(crate) fn valid_span_order(text: &str, spans: &[TextSpan]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..spans.len()).collect();
    if !spans.is_sorted_by_key(|span| span.range.start) {
        order.sort_by_key(|&at| spans[at].range.start);
    }
    let mut at = 0;
    order.retain(|&index| {
        let range = &spans[index].range;
        let fits = range.start >= at
            && range.start < range.end
            && range.end <= text.len()
            && text.is_char_boundary(range.start)
            && text.is_char_boundary(range.end);
        if fits {
            at = range.end;
        }
        fits
    });
    order
}

type Inked = (Range<usize>, Color);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inks(Option<Arc<[Inked]>>);

impl Inks {
    #[must_use]
    pub fn from_spans(text: &str, spans: &[TextSpan]) -> Self {
        if spans.iter().all(|span| span.color.is_none()) {
            return Self::default();
        }
        let inked: Vec<Inked> = valid_span_order(text, spans)
            .into_iter()
            .filter_map(|index| {
                let span = &spans[index];
                Some((span.range.clone(), span.color?))
            })
            .collect();
        Self((!inked.is_empty()).then(|| inked.into()))
    }

    fn ranges(&self) -> &[Inked] {
        self.0.as_deref().unwrap_or_default()
    }

    #[must_use]
    pub fn at(&self, offset: usize) -> Option<Color> {
        let ranges = self.ranges();
        let after = ranges.partition_point(|(range, _)| range.start <= offset);
        let (range, color) = ranges.get(after.checked_sub(1)?)?;
        range.contains(&offset).then_some(*color)
    }

    #[must_use]
    pub fn differing(&self, other: &Inks) -> Option<Range<usize>> {
        let bounds = || {
            self.ranges()
                .iter()
                .chain(other.ranges())
                .flat_map(|(range, _)| [range.start, range.end])
        };
        let differs = |at: &usize| self.at(*at) != other.at(*at);
        let start = bounds().filter(differs).min()?;
        let last = bounds().filter(differs).max()?;
        let end = bounds().filter(|bound| *bound > last).min();
        Some(start..end.unwrap_or(last + 1))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_none()
    }
}
