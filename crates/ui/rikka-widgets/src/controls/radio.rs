use rikka_types::Color;
use rikka_ui::{
    Action, Behavior, Element, Env, Event, EventCtx, IntoHandler, Layout, PaintCtx, Quad, Style,
    Styled, View, Widget, motion,
};

use crate::control::ControlSize;
use crate::controls::choice;
use crate::controls::press::Press;
use crate::controls::ripple::Ripple;

const BORDER: f32 = 1.5;

const DOT_RATIO: f32 = 0.4;

#[must_use]
pub struct Radio {
    selected: bool,
    label: Option<Element>,
    size: ControlSize,
    on_select: Action,
    enabled: bool,
    style: Style,
}

pub fn radio(selected: bool) -> Radio {
    Radio {
        selected,
        label: None,
        size: ControlSize::default(),
        on_select: Action::default(),
        enabled: true,
        style: Style::default(),
    }
}

impl Radio {
    pub fn label(mut self, label: impl View) -> Self {
        self.label = Some(label.build());
        self
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn on_select<Marker>(mut self, on_select: impl IntoHandler<(), (), Marker>) -> Self {
        self.on_select = Action::new(on_select);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    fn is_active(&self) -> bool {
        self.on_select.active(self.enabled)
    }
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct RadioState {
    press: Press,
    ripple: Ripple,
}

impl Widget for Radio {
    type State = RadioState;

    fn layout(&self) -> Layout {
        choice::layout(self.size, &self.style)
    }

    fn surroundings(&self, env: &mut Env) {
        let theme = rikka_ui::theme();
        env.font_size = self.size.font_size(&theme);
        if !self.enabled {
            env.text_color = theme.disabled(env.text_color);
        }
    }

    fn paint(&self, state: &mut RadioState, ctx: &mut PaintCtx<'_>) {
        let active = self.is_active();
        let ring_rect = choice::paint_row(ctx, &mut state.ripple, &state.press, self.size, active);

        let edge = ring_rect.width();
        let radius = edge / 2.0;
        let palette = ctx.theme().palette();
        let (target, outline, mark) = {
            let theme = ctx.theme();
            let dim = |color| theme.dim(color, !self.enabled);
            match self.selected {
                true => (dim(palette.accent), None, dim(palette.on_accent)),
                false => (
                    Color::TRANSPARENT,
                    Some(dim(palette.text_muted)),
                    Color::TRANSPARENT,
                ),
            }
        };

        let fill = ctx.animate_curve("ring", target, motion::COLOR);
        let mut quad = Quad::new(ring_rect).fill(fill).radius(radius);
        if let Some(outline) = outline {
            quad = quad.stroke(BORDER, outline);
        }
        ctx.draw_quad(quad);

        let dot = ctx.animate_curve(
            "dot",
            if self.selected { edge * DOT_RATIO } else { 0.0 },
            motion::TOGGLE,
        );
        if dot > 0.0 {
            ctx.draw_circle(ring_rect.center(), dot, mark);
        }
    }

    fn event(&self, state: &mut RadioState, event: &Event, ctx: &mut EventCtx<'_>) {
        if state.press.update(event, self.is_active(), ctx) && !self.selected {
            ctx.act(&self.on_select, ());
        }
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::TRACKS_HOVER | Behavior::FOCUSABLE.when(self.on_select.active(self.enabled))
    }

    fn same(&self, other: &Self) -> bool {
        self.selected == other.selected
            && self.size == other.size
            && self.on_select == other.on_select
            && self.enabled == other.enabled
            && self.style == other.style
    }
}

impl std::fmt::Debug for Radio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Radio")
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}

impl View for Radio {
    fn build(mut self) -> Element {
        let children = self.label.take().into_iter().collect();
        Element::with_children(self, children)
    }
}

impl Styled for Radio {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}
