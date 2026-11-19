use rikka_types::{Point, Rect};
use rikka_ui::{Element, PaintCtx, View, Widget};

#[derive(Debug)]
pub struct Scaled {
    factor: f32,
}

pub fn scaled(child: impl View, factor: f32) -> Element {
    Element::with_children(
        Scaled {
            factor: factor.max(0.0),
        },
        vec![child.build()],
    )
}

impl Widget for Scaled {
    type State = ();

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        if self.factor != 1.0 {
            ctx.set_subtree_scale(self.factor, ctx.bounds().origin);
        }
    }

    fn hit_test(&self, _bounds: Rect, _point: Point) -> bool {
        false
    }

    fn same(&self, other: &Self) -> bool {
        self.factor == other.factor
    }
}
