use rikka_ui::{Action, Event, EventCtx, IntoHandler, Layout, PaintCtx, Style, Styled, Widget};

use crate::controls::press::Press;

#[must_use]
#[derive(Widget)]
pub struct Scrim {
    dismiss: Action,
    #[style]
    style: Style,
}

pub fn scrim() -> Scrim {
    Scrim {
        dismiss: Action::default(),
        style: Style::default(),
    }
    .position(0.0, 0.0)
    .fill()
}

impl Scrim {
    pub fn on_dismiss<Marker>(mut self, dismiss: impl IntoHandler<(), (), Marker>) -> Self {
        self.dismiss = Action::new(dismiss);
        self
    }
}

#[derive(Debug, Default)]
pub struct ScrimState {
    press: Press,
}

impl Widget for Scrim {
    type State = ScrimState;

    fn layout(&self) -> Layout {
        Layout::DEFAULT.with(&self.style)
    }

    fn paint(&self, _state: &mut ScrimState, ctx: &mut PaintCtx<'_>) {
        ctx.draw_scrim(ctx.bounds());
    }

    fn event(&self, state: &mut ScrimState, event: &Event, ctx: &mut EventCtx<'_>) {
        state.press.set_declines_focus(true);
        if state.press.update(event, true, ctx) {
            ctx.act(&self.dismiss, ());
        }
        if matches!(
            event,
            Event::PointerPressed { .. } | Event::PointerReleased { .. } | Event::Scroll { .. }
        ) {
            ctx.set_handled();
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.dismiss == other.dismiss && self.style == other.style
    }
}
