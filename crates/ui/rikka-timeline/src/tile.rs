use std::time::Instant;

use rikka_types::{CursorShape, Point, Rect};
use rikka_ui::{
    Action, Behavior, Corner, Element, Event, EventCtx, IntoHandler, Label, PaintCtx,
    PointerButton, Quad, Styled, TextRun, View, Widget,
};

use crate::plot::CurvePlot;

pub const TILE_WIDTH: f32 = 48.0;
const HEIGHT: f32 = 30.0;
const INSET: f32 = 5.0;
const MARGIN: f32 = 0.05;
const STROKE: f32 = 1.5;
const DOT: f32 = 5.0;
const TRAVEL: f32 = 1.0;
const REST: f32 = 0.4;
const HOVER: f32 = 0.08;
const SELECTED: f32 = 0.18;
const DISABLED: f32 = 0.4;
const CAPTION: f32 = 11.5;
const CAPTION_LINE: f32 = 16.0;

#[must_use]
#[derive(Debug)]
pub struct CurveTile {
    samples: Vec<f32>,
    selected: bool,
    enabled: bool,
    caption: Option<Label>,
    press: Action<()>,
}

#[derive(Debug, Default)]
pub struct Hovering {
    since: Option<Instant>,
}

pub fn curve_tile(samples: Vec<f32>) -> CurveTile {
    CurveTile {
        samples,
        selected: false,
        enabled: true,
        caption: None,
        press: Action::default(),
    }
}

impl CurveTile {
    pub fn selected(self, selected: bool) -> CurveTile {
        CurveTile { selected, ..self }
    }

    pub fn enabled(self, enabled: bool) -> CurveTile {
        CurveTile { enabled, ..self }
    }

    pub fn caption(self, caption: Label) -> CurveTile {
        CurveTile {
            caption: Some(caption),
            ..self
        }
    }

    pub fn on_press<Marker>(self, press: impl IntoHandler<(), (), Marker>) -> CurveTile {
        CurveTile {
            press: Action::new(press),
            ..self
        }
    }

    fn points(&self) -> Vec<Point> {
        let last = self.samples.len().saturating_sub(1).max(1) as f32;
        self.samples
            .iter()
            .enumerate()
            .map(|(index, value)| Point::new(index as f32 / last, *value))
            .collect()
    }

    fn value_at(&self, along: f32) -> f32 {
        let last = self.samples.len().saturating_sub(1);
        let at = along.clamp(0.0, 1.0) * last as f32;
        let below = (at.floor() as usize).min(last);
        let above = (below + 1).min(last);
        let (low, high) = (self.samples[below], self.samples[above]);
        low + (high - low) * (at - below as f32)
    }

    fn plot(&self, bounds: Rect) -> CurvePlot {
        let inner = bounds.inflate(-INSET);
        let area = match self.caption {
            Some(_) => Rect::new(inner.x() + INSET, inner.y(), inner.height(), inner.height()),
            None => inner,
        };
        CurvePlot::fitting(area, self.samples.iter().copied(), MARGIN)
    }
}

impl Widget for CurveTile {
    type State = Hovering;

    fn paint(&self, state: &mut Hovering, ctx: &mut PaintCtx<'_>) {
        let palette = ctx.palette();
        let bounds = ctx.bounds();
        let live = self.enabled && ctx.hovered();
        let ground = match (self.selected, live) {
            (true, _) => palette.accent.scale_alpha(SELECTED),
            (false, true) => palette.text.scale_alpha(HOVER),
            (false, false) => palette.surface_variant,
        };
        let edge = match self.selected {
            true => palette.accent,
            false => palette.outline,
        };
        ctx.draw_quad(
            Quad::new(bounds)
                .fill(ground)
                .stroke(1.0, edge)
                .radius(Corner::Detail),
        );
        if self.samples.is_empty() {
            return;
        }
        let plot = self.plot(bounds);
        let ink = match (self.enabled, self.selected) {
            (false, _) => palette.text_muted.scale_alpha(DISABLED),
            (true, true) => palette.accent,
            (true, false) => palette.text,
        };
        plot.line(ctx, &self.points(), STROKE, ink);
        if let Some(caption) = &self.caption {
            let left = plot.area().max_x() + INSET * 2.0;
            ctx.draw_text(
                Rect::new(
                    left,
                    bounds.center().y - CAPTION_LINE / 2.0,
                    (bounds.max_x() - left - INSET).max(0.0),
                    CAPTION_LINE,
                ),
                TextRun::new(caption.as_str(), CAPTION).no_wrap(),
                palette.text,
            );
        }
        if !live {
            state.since = None;
            return;
        }
        let since = *state.since.get_or_insert(ctx.now());
        let elapsed = ctx.now().duration_since(since).as_secs_f32() % (TRAVEL + REST);
        let along = (elapsed / TRAVEL).min(1.0);
        ctx.draw_circle(
            plot.place(Point::new(along, self.value_at(along))),
            DOT,
            palette.accent,
        );
        ctx.request_frame();
    }

    fn event(&self, _state: &mut Hovering, event: &Event, ctx: &mut EventCtx<'_>) {
        match event {
            Event::PointerEntered | Event::PointerExited => ctx.request_paint(),
            Event::PointerPressed {
                button: PointerButton::Primary,
                ..
            } if self.enabled => {
                ctx.set_handled();
                ctx.keep_focus();
                ctx.act(&self.press, ());
            }
            _ => {}
        }
    }

    fn behavior(&self, _state: &Hovering) -> Behavior {
        Behavior::TRACKS_HOVER
    }

    fn cursor(&self, _state: &Hovering, _bounds: Rect, _point: Point) -> Option<CursorShape> {
        self.enabled.then_some(CursorShape::Pointer)
    }

    fn same(&self, other: &CurveTile) -> bool {
        self.samples == other.samples
            && self.selected == other.selected
            && self.enabled == other.enabled
            && self.caption == other.caption
    }

    fn debug_name(&self) -> &'static str {
        "CurveTile"
    }
}

impl View for CurveTile {
    fn build(self) -> Element {
        let captioned = self.caption.is_some();
        let element = Element::new(self).height(HEIGHT);
        match captioned {
            true => element.grow(1.0),
            false => element.width(TILE_WIDTH),
        }
    }
}
