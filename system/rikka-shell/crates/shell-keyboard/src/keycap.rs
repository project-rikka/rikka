use rikka_theme::{Corner, Theme};
use rikka_types::PointerButton;
use rikka_ui::{
    Align, Behavior, Color, Curve, Element, Env, Event, EventCtx, Handler, IntoHandler, Justify,
    Layout, Length, PaintCtx, Quad, Spring, View, Widget, theme,
};

use crate::latch::Latch;

const HOVER: f32 = 0.08;
const DOWN: f32 = 0.32;
const LATCHED: f32 = 0.28;

const FILL: Curve = Curve::Spring(Spring::FAST_EFFECTS);

pub(crate) struct Keycap {
    child: Element,
    width: f32,
    height: f32,
    latch: Latch,
    on_press: Handler,
    on_release: Handler,
}

#[derive(Debug, Default)]
pub(crate) struct KeycapState {
    down: bool,
}

pub(super) fn keycap<Pressed, Released>(
    child: impl View,
    width: f32,
    height: f32,
    on_press: impl IntoHandler<(), (), Pressed>,
    on_release: impl IntoHandler<(), (), Released>,
) -> Keycap {
    Keycap {
        child: child.build(),
        width,
        height,
        latch: Latch::Off,
        on_press: on_press.into_handler(),
        on_release: on_release.into_handler(),
    }
}

impl Keycap {
    pub(super) fn latch(mut self, latch: Latch) -> Self {
        self.latch = latch;
        self
    }

    fn fill(&self, theme: &Theme, down: bool, hovered: bool) -> Color {
        let palette = theme.palette();
        if self.latch == Latch::Locked {
            return palette.accent;
        }
        let resting = match self.latch {
            Latch::Once => palette.surface_variant.mix(palette.accent, LATCHED),
            _ => palette.surface_variant,
        };
        match (down, hovered) {
            (true, _) => resting.mix(palette.accent, DOWN),
            (false, true) => resting.mix(palette.text, HOVER),
            (false, false) => resting,
        }
    }

    fn foreground(&self, theme: &Theme) -> Color {
        let palette = theme.palette();
        match self.latch {
            Latch::Locked => palette.on_accent,
            _ => palette.text,
        }
    }
}

impl Widget for Keycap {
    type State = KeycapState;

    fn layout(&self) -> Layout {
        Layout::row()
            .align_items(Align::Center)
            .justify(Justify::Center)
            .size(Length::px(self.width), Length::px(self.height))
            .fixed()
    }

    fn surroundings(&self, env: &mut Env) {
        env.text_color = self.foreground(&theme());
    }

    fn paint(&self, state: &mut KeycapState, ctx: &mut PaintCtx<'_>) {
        let target = self.fill(&theme(), state.down, ctx.hovered());
        let fill = ctx.animate_curve("fill", target, FILL);
        ctx.draw_quad(Quad::new(ctx.bounds()).fill(fill).radius(Corner::Item));
    }

    fn event(&self, state: &mut KeycapState, event: &Event, ctx: &mut EventCtx<'_>) {
        match event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } if ctx.bounds().contains(*position) => {
                state.down = true;
                ctx.capture_pointer();
                ctx.set_handled();
                ctx.request_paint();
                ctx.call(&self.on_press, ());
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } if state.down => {
                state.down = false;
                ctx.release_pointer();
                ctx.set_handled();
                ctx.request_paint();
                ctx.call(&self.on_release, ());
            }
            Event::PointerLeft if state.down => {
                state.down = false;
                ctx.release_pointer();
                ctx.request_paint();
                ctx.call(&self.on_release, ());
            }
            _ => {}
        }
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::TRACKS_HOVER
    }

    fn debug_name(&self) -> &'static str {
        "Keycap"
    }
}

impl std::fmt::Debug for Keycap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keycap")
            .field("width", &self.width)
            .field("latch", &self.latch)
            .finish_non_exhaustive()
    }
}

impl View for Keycap {
    fn build(mut self) -> Element {
        let child = std::mem::replace(&mut self.child, Element::empty());
        Element::with_children(self, vec![child])
    }
}
