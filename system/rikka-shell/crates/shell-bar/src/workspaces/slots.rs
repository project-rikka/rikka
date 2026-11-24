use rikka_theme::Theme;
use rikka_types::PointerButton;
use rikka_ui::{
    Axis, Color, Curve, Element, Event, EventCtx, Handler, IntoHandler, Layout, Length, PaintCtx,
    Point, Quad, Rect, Size, Spring, View, Widget, theme,
};

use crate::axis;

const PILL: f32 = 24.0;

const PILL_THICKNESS: f32 = 14.0;

const DOT: [f32; 3] = [8.0, 6.0, 4.0];

const GAP: f32 = 6.0;

const EMPTY: f32 = 0.3;
const HOVERED: f32 = 0.6;

const INDICATOR: Curve = Curve::Spring(Spring::DEFAULT_SPATIAL);
pub(crate) const TOGGLE: Curve = Curve::Spring(Spring::FAST_SPATIAL);
const COLOR: Curve = Curve::Spring(Spring::DEFAULT_EFFECTS);

pub(crate) struct Slots {
    showing: usize,
    holding: Vec<bool>,
    axis: Axis,
    on_show: Handler<(usize,)>,
}

#[derive(Debug, Default)]
pub(crate) struct SlotsState {
    hovered: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Mark {
    along: f32,
    across: f32,
}

impl Slots {
    pub(crate) fn new<Marker>(
        showing: usize,
        holding: Vec<bool>,
        axis: Axis,
        on_show: impl IntoHandler<(usize,), (), Marker>,
    ) -> Slots {
        Slots {
            showing,
            holding,
            axis,
            on_show: on_show.into_handler(),
        }
    }

    fn count(&self) -> usize {
        self.holding.len()
    }

    fn places(&self) -> impl Iterator<Item = (f32, Size)> + '_ {
        marks(self.showing, self.count()).scan(0.0, |start, mark| {
            let middle = *start + mark.along / 2.0;
            *start += mark.along + GAP;
            Some((middle, axis::size(self.axis, mark.along, mark.across)))
        })
    }

    fn colour(&self, index: usize, theme: &Theme, hovered: bool) -> Color {
        let accent = theme.palette().accent;
        if index + 1 == self.showing || self.holding.get(index).copied().unwrap_or(false) {
            return accent;
        }
        accent.scale_alpha(if hovered { HOVERED } else { EMPTY })
    }

    fn mark_at(&self, bounds: Rect, at: Point) -> Option<usize> {
        if !bounds.contains(at) {
            return None;
        }
        let local = self.axis.coord(at) - self.axis.coord(bounds.origin);
        let mut edge = 0.0;
        marks(self.showing, self.count()).position(|mark| {
            edge += mark.along + GAP;
            local < edge
        })
    }

    fn stepped(&self, step: i32) -> Option<usize> {
        let count = i32::try_from(self.count()).ok()?;
        if count == 0 {
            return None;
        }
        let showing = i32::try_from(self.showing).ok()? - 1;
        usize::try_from((showing + step).rem_euclid(count) + 1).ok()
    }
}

fn marks(showing: usize, count: usize) -> impl Iterator<Item = Mark> {
    (1..=count).map(move |number| {
        if number == showing {
            return Mark {
                along: PILL,
                across: PILL_THICKNESS,
            };
        }
        let away = number.abs_diff(showing);
        let side = DOT[(away - 1).min(DOT.len() - 1)];
        Mark {
            along: side,
            across: side,
        }
    })
}

fn length(count: usize) -> f32 {
    (1..=count)
        .map(|showing| {
            marks(showing, count).map(|mark| mark.along).sum::<f32>() + GAP * (count - 1) as f32
        })
        .fold(0.0, f32::max)
}

impl Widget for Slots {
    type State = SlotsState;

    fn layout(&self) -> Layout {
        axis::sized(
            Layout::row(),
            self.axis,
            Length::px(length(self.count())),
            Length::FILL,
        )
    }

    fn paint(&self, state: &mut SlotsState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let start = self.axis.coord(bounds.origin);
        let middle = self.axis.across().coord(bounds.center());
        let hovered = state.hovered;
        for (index, (place, extent)) in self.places().enumerate() {
            let colour = self.colour(index, &theme(), hovered == Some(index));
            let at = start + ctx.animate_curve(("at", index), place, INDICATOR);
            let size = ctx.animate_curve(("size", index), extent, TOGGLE);
            let fill = ctx.animate_curve(("fill", index), colour, COLOR);
            ctx.draw_quad(
                Quad::new(Rect::from_center(self.axis.point(at, middle), size))
                    .fill(fill)
                    .radius(size.width.min(size.height) / 2.0),
            );
        }
    }

    fn event(&self, state: &mut SlotsState, event: &Event, ctx: &mut EventCtx<'_>) {
        let bounds = ctx.bounds();
        match event {
            Event::PointerMoved(at) => state.hovered = self.mark_at(bounds, *at),
            Event::PointerExited | Event::PointerLeft => state.hovered = None,
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                if let Some(index) = self.mark_at(bounds, *position) {
                    ctx.set_handled();
                    ctx.call(&self.on_show, (index + 1,));
                }
            }
            Event::Scroll { delta, .. } if delta.offset().y != 0.0 => {
                ctx.set_handled();
                if let Some(number) = self.stepped(if delta.offset().y > 0.0 { 1 } else { -1 }) {
                    ctx.call(&self.on_show, (number,));
                }
            }
            _ => {}
        }
    }

    fn debug_name(&self) -> &'static str {
        "Workspaces"
    }
}

impl std::fmt::Debug for Slots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Slots")
            .field("showing", &self.showing)
            .field("holding", &self.holding)
            .finish_non_exhaustive()
    }
}

impl View for Slots {
    fn build(self) -> Element {
        Element::new(self)
    }
}
