use rikka_types::{Point, Rect};
use rikka_ui::{Align, Behavior, Inset, Layout, Length, PaintCtx, Quad, Widget};

const HAIRLINE: f32 = 1.0;
const DIVIDER_HEIGHT: f32 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Placement {
    Fixed { divider: Option<f32> },
    Flexible { least: f32 },
    Offstage { x: f32 },
}

#[derive(Debug, PartialEq)]
pub(super) struct Slot {
    pub(super) placement: Placement,
}

pub(super) fn divider_space(gap: f32) -> f32 {
    gap + HAIRLINE
}

impl Widget for Slot {
    type State = ();

    fn layout(&self) -> Layout {
        let layout = Layout::row().align_items(Align::Center);
        match self.placement {
            Placement::Fixed { divider } => {
                layout
                    .fixed()
                    .padding(Inset::exact(0.0, 0.0, 0.0, divider.unwrap_or_default()))
            }
            Placement::Flexible { least } => {
                layout.grow(1.0).shrink(1.0).min_width(Length::px(least))
            }
            Placement::Offstage { x } => layout.fixed().placed_at(Point::new(x, 0.0)),
        }
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let Placement::Fixed { divider: Some(_) } = self.placement else {
            return;
        };
        let bounds = ctx.bounds();
        let line = Rect::new(
            bounds.x(),
            bounds.center().y - DIVIDER_HEIGHT / 2.0,
            HAIRLINE,
            DIVIDER_HEIGHT,
        );
        let outline = ctx.theme().palette().outline;
        ctx.draw_quad(Quad::new(line).fill(outline));
    }

    fn behavior(&self, _state: &()) -> Behavior {
        match self.placement {
            Placement::Offstage { .. } => Behavior::INERT,
            Placement::Fixed { .. } | Placement::Flexible { .. } => Behavior::NONE,
        }
    }

    fn same(&self, other: &Self) -> bool {
        self == other
    }
}
