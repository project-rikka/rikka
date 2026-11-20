use rikka_theme::Theme;
use rikka_types::{Color, Rect, Size};
use rikka_ui::{
    Action, Behavior, Event, EventCtx, IntoHandler, Layout, MeasureSpec, PaintCtx, Quad, Style,
    Widget, motion,
};

use crate::control::ControlSize;
use crate::controls::press::Press;
use crate::controls::ripple::engaged;

const PRESS_STRETCH: f32 = 0.25;

const HOVER_SWELL: f32 = 2.0;

const BORDER: f32 = 1.0;

#[must_use]
#[derive(PartialEq, Widget)]
pub struct Switch {
    #[debug]
    on: bool,
    #[debug]
    size: ControlSize,
    on_toggle: Action<(bool,)>,
    enabled: bool,
    #[style]
    style: Style,
}

pub fn switch(on: bool) -> Switch {
    Switch {
        on,
        size: ControlSize::default(),
        on_toggle: Action::default(),
        enabled: true,
        style: Style::default(),
    }
}

struct Geometry {
    width: f32,
    height: f32,
    knob: f32,
}

impl Geometry {
    fn inset(&self) -> f32 {
        (self.height - self.knob) / 2.0
    }

    fn travel(&self) -> f32 {
        self.width - self.knob - self.inset() * 2.0
    }
}

impl Switch {
    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn on_toggle<Marker>(mut self, on_toggle: impl IntoHandler<(bool,), (), Marker>) -> Self {
        self.on_toggle = Action::new(on_toggle);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    fn is_active(&self) -> bool {
        self.on_toggle.active(self.enabled)
    }

    fn geometry(&self) -> Geometry {
        match self.size {
            ControlSize::Small => Geometry {
                width: 38.0,
                height: 22.0,
                knob: 16.0,
            },
            ControlSize::Medium => Geometry {
                width: 46.0,
                height: 28.0,
                knob: 22.0,
            },
            ControlSize::Large => Geometry {
                width: 54.0,
                height: 32.0,
                knob: 26.0,
            },
        }
    }

    fn colors(&self, theme: &Theme) -> (Color, Color) {
        let palette = &theme.palette();
        let (track, knob) = if self.on {
            (palette.accent, palette.on_accent)
        } else {
            (palette.surface_variant, palette.text_muted)
        };
        (
            theme.dim(track, !self.enabled),
            theme.dim(knob, !self.enabled),
        )
    }
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct SwitchState {
    press: Press,
}

impl Widget for Switch {
    type State = SwitchState;

    fn layout(&self) -> Layout {
        Layout::row().fixed().with(&self.style)
    }

    fn measure(&self) -> Option<MeasureSpec> {
        let geometry = self.geometry();
        Some(MeasureSpec::Fixed(Size::new(
            geometry.width,
            geometry.height,
        )))
    }

    fn paint(&self, state: &mut SwitchState, ctx: &mut PaintCtx<'_>) {
        let geometry = self.geometry();
        let active = self.is_active();
        let down = active && state.press.is_down();

        let palette = ctx.theme().palette();
        let (track_target, knob_target) = self.colors(ctx.theme());
        let track_color = ctx.animate_curve("track", track_target, motion::COLOR);
        let knob_color = ctx.animate_curve("knob", knob_target, motion::COLOR);
        let offset = ctx.animate_curve("offset", if self.on { 1.0 } else { 0.0 }, motion::VALUE);
        let stretch = ctx.animate_curve(
            "stretch",
            if down {
                geometry.knob * PRESS_STRETCH
            } else {
                0.0
            },
            if down { motion::PRESS } else { motion::MORPH },
        );
        let swell = ctx.animate_curve(
            "swell",
            if engaged(ctx, active, down) {
                HOVER_SWELL
            } else {
                0.0
            },
            motion::MORPH,
        );

        let track = Rect::from_center(
            ctx.bounds().center(),
            Size::new(geometry.width, geometry.height),
        );
        let radius = geometry.height / 2.0;

        let mut quad = Quad::new(track).fill(track_color).radius(radius);
        if !self.on {
            quad = quad.stroke(BORDER, palette.outline);
        }
        ctx.draw_quad(quad);

        let inset = geometry.inset() - swell / 2.0;
        let diameter = geometry.knob + swell;
        let left = track.x() + inset + offset * geometry.travel() - stretch * offset;
        let knob = Rect::new(left, track.y() + inset, diameter + stretch, diameter);

        ctx.draw_quad(Quad::new(knob).fill(knob_color).radius(diameter / 2.0));
    }

    fn event(&self, state: &mut SwitchState, event: &Event, ctx: &mut EventCtx<'_>) {
        if state.press.update(event, self.is_active(), ctx) {
            ctx.act(&self.on_toggle, (!self.on,));
        }
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::TRACKS_HOVER | Behavior::FOCUSABLE.when(self.on_toggle.active(self.enabled))
    }

    fn same(&self, other: &Self) -> bool {
        self == other
    }
}
