use rikka_symbols::IconGlyph;
use rikka_text::Align as TextAlign;
use rikka_theme::Theme;
use rikka_types::{Color, Rect};
use rikka_ui::motion::indicator::{self, Indicator};
use rikka_ui::text::{Label, TextRun};
use rikka_ui::{PaintCtx, Quad, measure_text, motion};

use crate::control::ControlSize;
use crate::rows;

pub(crate) const PADDING: f32 = 3.0;

const GLYPH_RATIO: f32 = 1.25;
const LEAST_EMS: f32 = 2.0;
const GLYPH_GAP_EMS: f32 = 0.4;

pub(crate) enum Facet {
    Lettered(Label),
    Glyph(IconGlyph),
    Labelled(IconGlyph, Label),
}

fn glyph_size(font_size: f32) -> f32 {
    (font_size * GLYPH_RATIO).round()
}

fn glyph_gap(font_size: f32) -> f32 {
    (font_size * GLYPH_GAP_EMS).round()
}

fn lettered_width(label: &Label, font_size: f32) -> f32 {
    let run = TextRun::new(label.clone(), font_size).medium().no_wrap();
    measure_text(&run, None).width
}

fn paint_glyph(ctx: &mut PaintCtx<'_>, glyph: IconGlyph, x: f32, y: f32, size: f32, color: Color) {
    ctx.draw_icon(rows::glyph_rect(x, y, size), glyph.spec(size), color);
}

impl Facet {
    fn width(&self, font_size: f32) -> f32 {
        match self {
            Facet::Lettered(label) => lettered_width(label, font_size),
            Facet::Glyph(_) => glyph_size(font_size),
            Facet::Labelled(_, label) => {
                glyph_size(font_size) + glyph_gap(font_size) + lettered_width(label, font_size)
            }
        }
    }

    fn least(&self, font_size: f32) -> f32 {
        match self {
            Facet::Labelled(..) => glyph_size(font_size),
            Facet::Lettered(_) | Facet::Glyph(_) => self.width(font_size),
        }
    }

    fn paint(
        self,
        ctx: &mut PaintCtx<'_>,
        content: Rect,
        measured: f32,
        font_size: f32,
        color: Color,
    ) {
        let size = glyph_size(font_size);
        let middle = content.center();
        match self {
            Facet::Lettered(label) => {
                let run = TextRun::new(label, font_size)
                    .medium()
                    .no_wrap()
                    .align(TextAlign::Center);
                rows::draw_cell(ctx, content, content.x(), content.width(), run, color);
            }
            Facet::Glyph(glyph) => {
                paint_glyph(ctx, glyph, middle.x - size / 2.0, middle.y, size, color);
            }
            Facet::Labelled(glyph, label) => {
                let gap = glyph_gap(font_size);
                let whole = measured;
                let text = whole - size - gap;
                if whole > content.width() {
                    paint_glyph(ctx, glyph, middle.x - size / 2.0, middle.y, size, color);
                    return;
                }
                let start = middle.x - whole / 2.0;
                paint_glyph(ctx, glyph, start, middle.y, size, color);
                let run = TextRun::new(label, font_size).medium().no_wrap();
                let x = start + size + gap;
                rows::draw_cell(ctx, content, x, text, run, color);
            }
        }
    }
}

pub(crate) struct Track {
    size: ControlSize,
    font_size: f32,
    contents: Vec<f32>,
    leasts: Vec<f32>,
}

impl Track {
    pub(crate) fn new(
        size: ControlSize,
        theme: &Theme,
        facets: impl Iterator<Item = Facet>,
    ) -> Self {
        let font_size = size.font_size(theme);
        let (contents, leasts) = facets
            .map(|facet| (facet.width(font_size).ceil(), facet.least(font_size).ceil()))
            .unzip();
        Self {
            size,
            font_size,
            contents,
            leasts,
        }
    }

    pub(crate) fn height(size: ControlSize) -> f32 {
        size.height() + PADDING * 2.0
    }

    pub(crate) fn natural_width(&self) -> f32 {
        let widest = self.contents.iter().copied().fold(0.0, f32::max);
        (widest + self.inset()) * self.contents.len() as f32 + PADDING * 2.0
    }

    pub(crate) fn least_width(&self) -> f32 {
        let least = self.font_size * LEAST_EMS;
        self.leasts
            .iter()
            .map(|content| content.min(least) + self.inset())
            .sum::<f32>()
            + PADDING * 2.0
    }

    pub(crate) fn paint(
        &self,
        ctx: &mut PaintCtx<'_>,
        state: &mut TrackState,
        bounds: Rect,
        facet: impl Fn(usize) -> Facet,
        selected: usize,
        dimmed: bool,
    ) {
        let count = self.contents.len();
        if count == 0 {
            return;
        }
        let palette = ctx.theme().palette();
        let (radius, seat, active, idle, hover) = {
            let theme = ctx.theme();
            let dim = |color| theme.dim(color, dimmed);
            (
                self.size.radius(),
                dim(palette.selection),
                dim(palette.on_selection),
                dim(palette.text_muted),
                theme.hover_layer(),
            )
        };
        let spans = self.spans(bounds);

        ctx.draw_quad(
            Quad::new(bounds)
                .fill(palette.surface_variant)
                .radius(radius + PADDING),
        );

        let selected = selected.min(count - 1);
        let hovered = state
            .hovered
            .filter(|index| *index != selected)
            .map(|index| (index, index as f32));
        let hovering = state
            .hover
            .resolve(ctx, "track.hover", hovered, hover, motion::INDICATOR);
        if let Some((at, fill)) = hovering {
            ctx.draw_quad(Quad::new(spans.segment(at)).fill(fill).radius(radius));
        }

        let resting = selected as f32;
        let selection = state.selection.resolve(
            ctx,
            "track.selection",
            Some((selected, resting)),
            seat,
            motion::INDICATOR,
        );
        if let Some((at, fill)) = selection {
            ctx.draw_quad(Quad::new(spans.segment(at)).fill(fill).radius(radius));
        }

        let offset = selection.map_or(resting, |(at, _)| at);
        for index in 0..count {
            let color = idle.mix(active, indicator::emphasis(offset, index as f32, 1.0));
            let measured = self.contents[index];
            facet(index).paint(ctx, spans.content(index), measured, self.font_size, color);
        }
    }

    fn inset(&self) -> f32 {
        self.size.gap() * 2.0
    }

    pub(crate) fn spans(&self, bounds: Rect) -> Spans<'_> {
        let inner = bounds.inflate(-PADDING);
        let widths = apportion(&self.contents, self.inset(), inner.width());
        Spans::new(inner, widths, &self.contents, self.size.gap())
    }
}

fn apportion(contents: &[f32], inset: f32, room: f32) -> Vec<f32> {
    let mut sorted: Vec<f32> = contents.iter().map(|content| content + inset).collect();
    let total: f32 = sorted.iter().sum();
    let fit: fn(f32, f32) -> f32 = if room >= total {
        sorted.sort_by(f32::total_cmp);
        f32::max
    } else {
        sorted.sort_by(|a, b| b.total_cmp(a));
        f32::min
    };
    let level = level(&sorted, total, room, fit);
    contents
        .iter()
        .map(|content| fit(content + inset, level))
        .collect()
}

fn level(sorted: &[f32], total: f32, room: f32, fit: fn(f32, f32) -> f32) -> f32 {
    let mut rest = total;
    for (index, width) in sorted.iter().enumerate() {
        rest -= width;
        let level = (room - rest) / (index + 1) as f32;
        if sorted
            .get(index + 1)
            .is_none_or(|next| fit(*next, level) == *next)
        {
            return level;
        }
    }
    room
}

pub(crate) struct Spans<'a> {
    inner: Rect,
    starts: Vec<f32>,
    widths: Vec<f32>,
    contents: &'a [f32],
    gap: f32,
}

impl<'a> Spans<'a> {
    fn new(inner: Rect, widths: Vec<f32>, contents: &'a [f32], gap: f32) -> Self {
        let starts = widths
            .iter()
            .scan(inner.x(), |x, width| {
                let start = *x;
                *x += width;
                Some(start)
            })
            .collect();
        Self {
            inner,
            starts,
            widths,
            contents,
            gap,
        }
    }

    pub(crate) fn segment(&self, index: f32) -> Rect {
        let last = self.widths.len() - 1;
        let from = (index.floor().max(0.0) as usize).min(last);
        let to = (from + 1).min(last);
        let travel = index - from as f32;
        let width = self.widths[from] + (self.widths[to] - self.widths[from]) * travel;
        Rect::new(
            self.starts[from] + self.widths[from] * travel,
            self.inner.y(),
            width,
            self.inner.height(),
        )
    }

    pub(crate) fn at(&self, x: f32) -> usize {
        self.starts
            .iter()
            .rposition(|start| *start <= x)
            .unwrap_or(0)
    }

    pub(crate) fn is_elided(&self, index: usize) -> bool {
        self.contents[index] > self.content(index).width()
    }

    fn content(&self, index: usize) -> Rect {
        let segment = self.segment(index as f32);
        Rect::new(
            segment.x() + self.gap,
            segment.y(),
            segment.width() - self.gap * 2.0,
            segment.height(),
        )
    }
}

#[derive(Debug, Default)]
pub(crate) struct TrackState {
    selection: Indicator,
    hover: Indicator,
    hovered: Option<usize>,
}

impl TrackState {
    pub(crate) fn hover(&mut self, segment: Option<usize>) -> bool {
        let changed = self.hovered != segment;
        self.hovered = segment;
        changed
    }
}
