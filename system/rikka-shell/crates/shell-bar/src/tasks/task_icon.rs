use rikka_types::PointerButton;
use rikka_ui::{
    Behavior, Curve, Element, Event, EventCtx, Handler, Layout, Length, PaintCtx, Point, Rect,
    Size, View, Widget, motion, palette,
};

use crate::tasks::Art;
use crate::tasks::task_list::{Aim, Touch};

const ICON_SHARE: f32 = 22.0 / 28.0;
const HOVERED: f32 = 1.1;
const PRESSED: f32 = 0.92;
const MINIMIZED: f32 = 0.45;

const ZOOM: &str = "zoom";
const PRESENCE: &str = "presence";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lift {
    Resting,
    Hovered,
    Pressed,
}

impl Lift {
    fn zoom(self) -> f32 {
        match self {
            Lift::Resting => 1.0,
            Lift::Hovered => HOVERED,
            Lift::Pressed => PRESSED,
        }
    }

    fn curve(self) -> Curve {
        match self {
            Lift::Pressed => motion::PRESS,
            Lift::Resting | Lift::Hovered => motion::LIFT,
        }
    }
}

pub(crate) struct TaskIcon {
    pub(crate) space: usize,
    pub(crate) key: String,
    pub(crate) icon: Art,
    pub(crate) minimized: bool,
    pub(crate) size: Size,
    pub(crate) on_touch: Handler<(Touch,)>,
}

#[derive(Debug, Default)]
pub(crate) struct TaskIconState {
    hovered: bool,
    pressed: bool,
}

impl TaskIcon {
    fn chose(&self, bounds: Rect, button: PointerButton) -> Touch {
        Touch::Chose {
            space: self.space,
            key: self.key.clone(),
            button,
            at: Point::new(bounds.center().x, bounds.max_y()),
        }
    }
}

impl Widget for TaskIcon {
    type State = TaskIconState;

    fn layout(&self) -> Layout {
        Layout::row()
            .size(Length::px(self.size.width), Length::px(self.size.height))
            .fixed()
    }

    fn paint(&self, state: &mut TaskIconState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let lift = match (state.hovered, state.pressed) {
            (true, true) => Lift::Pressed,
            (true, false) => Lift::Hovered,
            (false, _) => Lift::Resting,
        };
        let zoom = ctx.animate_curve(ZOOM, lift.zoom(), lift.curve());
        let presence = match self.minimized {
            true => MINIMIZED,
            false => 1.0,
        };
        let presence = ctx.animate_curve(PRESENCE, presence, motion::FILL);
        let side = bounds.width() * ICON_SHARE;
        let art = Rect::from_center(bounds.center(), Size::new(side, side));
        self.icon.paint(ctx, art, zoom, &palette(), presence);
    }

    fn event(&self, state: &mut TaskIconState, event: &Event, ctx: &mut EventCtx<'_>) {
        let bounds = ctx.bounds();
        match event {
            Event::PointerEntered => {
                state.hovered = true;
                ctx.request_paint();
                let aim = Aim {
                    space: self.space,
                    key: self.key.clone(),
                    centre: bounds.center().x,
                };
                ctx.call(&self.on_touch, (Touch::Aimed(aim),));
            }
            Event::PointerExited | Event::PointerLeft => {
                state.hovered = false;
                ctx.request_paint();
            }
            Event::PointerPressed { button, .. } => {
                ctx.set_handled();
                if *button != PointerButton::Primary {
                    ctx.call(&self.on_touch, (self.chose(bounds, *button),));
                    return;
                }
                state.pressed = true;
                ctx.capture_pointer();
                ctx.request_paint();
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                if !std::mem::take(&mut state.pressed) {
                    return;
                }
                ctx.release_pointer();
                ctx.set_handled();
                ctx.request_paint();
                if bounds.contains(*position) {
                    ctx.call(
                        &self.on_touch,
                        (self.chose(bounds, PointerButton::Primary),),
                    );
                }
            }
            _ => {}
        }
    }

    fn behavior(&self, _state: &TaskIconState) -> Behavior {
        Behavior::TRACKS_HOVER
    }

    fn debug_name(&self) -> &'static str {
        "TaskIcon"
    }
}

impl std::fmt::Debug for TaskIcon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskIcon")
            .field("key", &self.key)
            .field("minimized", &self.minimized)
            .finish_non_exhaustive()
    }
}

impl View for TaskIcon {
    fn build(self) -> Element {
        Element::new(self)
    }
}
