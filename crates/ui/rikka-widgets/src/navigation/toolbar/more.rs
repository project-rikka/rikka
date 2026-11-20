use rikka_symbols::MaterialSymbol;
use rikka_text::IconSpec;
use rikka_types::{Key, Point, PointerButton, Rect, Size};
use rikka_ui::{
    Behavior, Event, EventCtx, Layout, Length, PaintCtx, Quad, Tooltip, Widget, motion, tr,
};

use crate::control::ControlSize;
use crate::controls::ripple::state_layer;
use crate::navigation::menu::{Activation, ContextMenu, MenuItem};

pub(super) struct More {
    pub(super) items: Vec<MenuItem>,
    pub(super) size: ControlSize,
}

impl More {
    pub(super) fn width(size: ControlSize) -> f32 {
        size.height()
    }

    fn glyph(&self) -> f32 {
        match self.size {
            ControlSize::Small => 18.0,
            ControlSize::Medium | ControlSize::Large => 24.0,
        }
    }

    fn open(&self, state: &mut MoreState, activation: Activation, ctx: &mut EventCtx<'_>) {
        ctx.set_handled();
        state
            .menu
            .drop_down(self.items.clone(), ctx.bounds(), activation, ctx);
    }
}

#[derive(Debug, Default)]
pub(super) struct MoreState {
    menu: ContextMenu,
}

impl Widget for More {
    type State = MoreState;

    fn layout(&self) -> Layout {
        let side = Length::px(Self::width(self.size));
        Layout::row().size(side, side).fixed()
    }

    fn paint(&self, state: &mut MoreState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let open = state.menu.is_open();
        let layer = state_layer(ctx, true, open);
        if let Some(fill) = ctx.fade("layer", layer, motion::HOVER) {
            ctx.draw_quad(Quad::new(bounds).fill(fill).radius(self.size.radius()));
        }
        let glyph = self.glyph();
        let text = ctx.theme().palette().text;
        ctx.draw_icon(
            Rect::from_center(bounds.center(), Size::new(glyph, glyph)),
            IconSpec::new(MaterialSymbol::MORE_HORIZ.codepoint(), glyph),
            text,
        );
        state.menu.paint(ctx);
    }

    fn event(&self, state: &mut MoreState, event: &Event, ctx: &mut EventCtx<'_>) {
        let open = state.menu.is_open();
        if state.menu.event(event, ctx) {
            if open && !state.menu.is_open() {
                ctx.return_focus();
            }
            return;
        }
        match *event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } if ctx.bounds().contains(position) => self.open(state, Activation::Pointer, ctx),
            Event::KeyPressed {
                key: Key::Enter | Key::Space | Key::ArrowDown,
                ..
            } if ctx.focused() => self.open(state, Activation::Keyboard, ctx),
            _ => {}
        }
    }

    fn behavior(&self, _state: &MoreState) -> Behavior {
        Behavior::TRACKS_HOVER | Behavior::FOCUSABLE | Behavior::NO_TAB_STOP
    }

    fn tooltip_at(
        &self,
        _state: &MoreState,
        bounds: Rect,
        _point: Point,
    ) -> Option<(Tooltip, Rect)> {
        Some((Tooltip::new(tr!("widgets.toolbar.more")), bounds))
    }
}

impl std::fmt::Debug for More {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("More")
            .field("items", &self.items.len())
            .finish_non_exhaustive()
    }
}
