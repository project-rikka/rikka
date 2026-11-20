use rikka_app::prelude::*;
use rikka_ui::motion;

const RAISED: f32 = 1.035;

pub(crate) fn lift(child: impl View) -> Element {
    Element::with_children(Lift, vec![child.build()])
}

#[derive(Debug)]
struct Lift;

impl Widget for Lift {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::DEFAULT
    }

    fn paint(&self, (): &mut (), ctx: &mut PaintCtx<'_>) {
        let raised = ctx.animate_curve(
            "raised",
            if ctx.hovered() { RAISED } else { 1.0 },
            motion::HOVER,
        );
        ctx.set_subtree_scale(raised, ctx.bounds().center());
    }

    fn behavior(&self, (): &()) -> Behavior {
        Behavior::TRACKS_HOVER
    }

    fn same(&self, _other: &Self) -> bool {
        true
    }

    fn debug_name(&self) -> &'static str {
        "Lift"
    }
}
