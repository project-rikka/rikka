use std::cell::RefCell;
use std::ops::Range;
use std::sync::Arc;
use std::time::Instant;

use rikka_animation::Channel;
use rikka_text::TextSpan;
use rikka_types::Color;
use rikka_ui::{PaintCtx, motion};

type Pieces = Vec<(usize, Instant)>;

thread_local! {
    static FADING: RefCell<Vec<(Arc<str>, Pieces)>> = const { RefCell::new(Vec::new()) };
}

#[derive(Debug, Default)]
pub(crate) struct RevealState {
    previous: Option<Arc<str>>,
    pieces: Pieces,
}

impl RevealState {
    pub(crate) fn veil(&mut self, content: &Arc<str>, grows: bool, ctx: &mut PaintCtx<'_>) -> Veil {
        let now = ctx.now();
        let grown = match &self.previous {
            None => {
                let (pieces, from) = resume_fading(content, now);
                self.pieces = pieces;
                from.or(grows.then_some(0))
            }
            Some(previous) if Arc::ptr_eq(previous, content) || **previous == **content => None,
            Some(previous) => grows.then(|| grown_from(previous, content)),
        };
        let was = self.previous.replace(Arc::clone(content));

        self.pieces.retain(|(start, _)| *start < content.len());
        if let Some(from) = grown.filter(|from| *from < content.len())
            && self.pieces.last().is_none_or(|(last, _)| *last < from)
        {
            self.pieces.push((from, now));
        }

        let mut veiled = Vec::with_capacity(self.pieces.len());
        self.pieces.retain(|(start, born)| {
            let alpha = unveiled(now, *born);
            if alpha < 1.0 {
                veiled.push((*start, alpha));
            }
            alpha < 1.0
        });

        retain_fading(was.as_ref(), content, &self.pieces, now);
        if !veiled.is_empty() {
            ctx.request_frame();
        }
        Veil(veiled)
    }
}

fn unveiled(now: Instant, born: Instant) -> f32 {
    let mut channel = Channel::new(0.0);
    channel.advance(motion::UNVEIL, 1.0, now.saturating_duration_since(born));
    channel.value
}

fn is_fading(now: Instant) -> impl FnMut(&(Arc<str>, Pieces)) -> bool {
    move |(_, pieces)| pieces.iter().any(|(_, born)| unveiled(now, *born) < 1.0)
}

fn retain_fading(was: Option<&Arc<str>>, content: &Arc<str>, pieces: &Pieces, now: Instant) {
    FADING.with_borrow_mut(|fading| {
        if fading.is_empty() && pieces.is_empty() {
            return;
        }
        fading.retain(|(faded, _)| {
            !Arc::ptr_eq(faded, content) && !was.is_some_and(|was| Arc::ptr_eq(faded, was))
        });
        fading.retain(is_fading(now));
        if !pieces.is_empty() {
            fading.push((Arc::clone(content), pieces.clone()));
        }
    });
}

fn resume_fading(content: &str, now: Instant) -> (Pieces, Option<usize>) {
    FADING.with_borrow_mut(|fading| {
        fading.retain(is_fading(now));
        fading
            .iter()
            .filter(|(faded, _)| !faded.is_empty() && content.starts_with(&**faded))
            .max_by_key(|(faded, _)| faded.len())
            .map_or((Pieces::new(), None), |(faded, pieces)| {
                let more = (faded.len() < content.len()).then_some(faded.len());
                (pieces.clone(), more)
            })
    })
}

fn grown_from(previous: &str, content: &str) -> usize {
    if content.starts_with(previous) {
        return previous.len();
    }
    let mut at = previous.len().min(content.len());
    while !content.is_char_boundary(at) {
        at += 1;
    }
    at
}

#[derive(Debug, Default, PartialEq)]
pub(crate) struct Veil(Vec<(usize, f32)>);

impl Veil {
    pub(crate) fn is_clear(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn alpha_at(&self, offset: usize) -> f32 {
        self.0
            .iter()
            .rev()
            .find(|(start, _)| *start <= offset)
            .map_or(1.0, |(_, alpha)| *alpha)
    }

    pub(crate) fn inks(&self, spans: &[TextSpan], base: Color, len: usize) -> Vec<TextSpan> {
        let Some(&(veiled, _)) = self.0.first() else {
            return spans.to_vec();
        };
        let coloured = coloured(spans, len);
        let mut inks: Vec<TextSpan> = coloured
            .iter()
            .filter(|(range, _)| range.start < veiled)
            .map(|(range, color)| inked(range.start..range.end.min(veiled), *color))
            .collect();

        let mut edges: Vec<usize> = self.0.iter().map(|(start, _)| *start).collect();
        for (range, _) in &coloured {
            edges.extend([range.start, range.end]);
        }
        edges.push(len);
        edges.retain(|edge| *edge >= veiled && *edge <= len);
        edges.sort_unstable();
        edges.dedup();

        for pair in edges.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            let color = coloured
                .iter()
                .find(|(range, _)| range.contains(&from))
                .map_or(base, |(_, color)| *color);
            inks.push(inked(from..to, color.scale_alpha(self.alpha_at(from))));
        }
        inks
    }
}

fn coloured(spans: &[TextSpan], len: usize) -> Vec<(Range<usize>, Color)> {
    let mut coloured: Vec<(Range<usize>, Color)> = spans
        .iter()
        .filter(|span| span.range.start < span.range.end && span.range.end <= len)
        .filter_map(|span| Some((span.range.clone(), span.color?)))
        .collect();
    coloured.sort_by_key(|(range, _)| range.start);
    let mut reached = 0;
    coloured.retain(|(range, _)| {
        let fits = range.start >= reached;
        if fits {
            reached = range.end;
        }
        fits
    });
    coloured
}

fn inked(range: Range<usize>, color: Color) -> TextSpan {
    TextSpan {
        color: Some(color),
        ..TextSpan::new(range)
    }
}
