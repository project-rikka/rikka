use rikka_theme::Theme;
use rikka_types::{Color, Rect, RoundedRect};
use rikka_ui::{
    Action, Align, Behavior, Element, Env, Event, EventCtx, Inset, IntoHandler, Justify, Layout,
    Length, PaintCtx, Quad, Style, Styled, View, Widget, motion,
};

use crate::control::ControlSize;
use crate::controls::press::Press;
use crate::controls::ripple::Ripple;
use crate::controls::spinner::spinner;

const PRESS_SCALE: f32 = 0.96;
const PRESS_ROUNDING: f32 = 0.6;

const BORDER: f32 = 1.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Ghost,
    Danger,
}

#[must_use]
pub struct Button {
    child: Element,
    variant: ButtonVariant,
    size: ControlSize,
    press: Action,
    enabled: bool,
    loading: bool,
    icon_only: bool,
    focusless: bool,
    style: Style,
}

pub fn button(child: impl View) -> Button {
    Button {
        child: child.build(),
        variant: ButtonVariant::default(),
        size: ControlSize::default(),
        press: Action::default(),
        enabled: true,
        loading: false,
        icon_only: false,
        focusless: false,
        style: Style::default(),
    }
}

struct Skin {
    fill: Color,
    border: Option<Color>,
    foreground: Color,
}

impl Button {
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn primary(self) -> Self {
        self.variant(ButtonVariant::Primary)
    }

    pub fn secondary(self) -> Self {
        self.variant(ButtonVariant::Secondary)
    }

    pub fn ghost(self) -> Self {
        self.variant(ButtonVariant::Ghost)
    }

    pub fn danger(self) -> Self {
        self.variant(ButtonVariant::Danger)
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn small(self) -> Self {
        self.control_size(ControlSize::Small)
    }

    pub fn large(self) -> Self {
        self.control_size(ControlSize::Large)
    }

    pub fn on_press<Marker>(mut self, press: impl IntoHandler<(), (), Marker>) -> Self {
        self.press = Action::new(press);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    pub fn icon_only(mut self) -> Self {
        self.icon_only = true;
        self
    }

    pub fn focusless(mut self) -> Self {
        self.focusless = true;
        self
    }

    fn is_active(&self) -> bool {
        self.press.active(self.enabled && !self.loading)
    }

    fn skin(&self, theme: &Theme) -> Skin {
        let palette = &theme.palette();
        let mut skin = match self.variant {
            ButtonVariant::Primary => Skin {
                fill: palette.accent,
                border: None,
                foreground: palette.on_accent,
            },
            ButtonVariant::Secondary => Skin {
                fill: palette.surface,
                border: Some(palette.outline),
                foreground: palette.text,
            },
            ButtonVariant::Ghost => Skin {
                fill: Color::TRANSPARENT,
                border: None,
                foreground: palette.text,
            },
            ButtonVariant::Danger => Skin {
                fill: palette.danger,
                border: None,
                foreground: palette.on_danger,
            },
        };
        let inert = !self.enabled || self.loading;
        skin.fill = theme.dim(skin.fill, inert);
        skin.border = skin.border.map(|color| theme.dim(color, inert));
        skin.foreground = theme.dim(skin.foreground, inert);
        skin
    }
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct ButtonState {
    press: Press,
    ripple: Ripple,
}

impl Widget for Button {
    type State = ButtonState;

    fn layout(&self) -> Layout {
        let height = self.size.height();
        let padding = match self.icon_only {
            true => 0.0,
            false => self.size.padding_x(),
        };
        let width = match self.icon_only {
            true => Length::px(height),
            false => Length::HUG,
        };
        Layout::row()
            .align_items(Align::Center)
            .justify(Justify::Center)
            .gap_exact(self.size.gap(), 0.0)
            .padding(Inset::xy(padding, 0.0))
            .size(width, Length::px(height))
            .fixed()
            .with(&self.style)
    }

    fn surroundings(&self, env: &mut Env) {
        let theme = rikka_ui::theme();
        env.text_color = self.skin(&theme).foreground;
        env.font_size = self.size.font_size(&theme);
    }

    fn paint(&self, state: &mut ButtonState, ctx: &mut PaintCtx<'_>) {
        let skin = self.skin(ctx.theme());
        let active = self.is_active();
        let down = active && state.press.is_down();

        let fill = ctx.animate_curve("fill", skin.fill, motion::COLOR);
        let release = if down { motion::PRESS } else { motion::MORPH };
        let scale = ctx.animate_curve("scale", if down { PRESS_SCALE } else { 1.0 }, release);
        let resting = self.size.radius();
        let radius = ctx.animate_curve(
            "radius",
            if down {
                resting * PRESS_ROUNDING
            } else {
                resting
            },
            release,
        );

        let rect = Rect::from_center(ctx.bounds().center(), ctx.bounds().size * scale);
        let mut quad = Quad::new(rect).fill(fill).radius(radius);
        if let Some(border) = skin.border {
            quad = quad.stroke(BORDER, border);
        }
        ctx.draw_quad(quad);

        state
            .ripple
            .paint(ctx, &state.press, RoundedRect::new(rect, radius), active);
    }

    fn event(&self, state: &mut ButtonState, event: &Event, ctx: &mut EventCtx<'_>) {
        state.press.set_declines_focus(self.focusless);
        if state.press.update(event, self.is_active(), ctx) {
            ctx.act(&self.press, ());
        }
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::TRACKS_HOVER
            | Behavior::FOCUSABLE.when(self.press.active(self.enabled) && !self.focusless)
    }

    fn same(&self, other: &Self) -> bool {
        self.variant == other.variant
            && self.size == other.size
            && self.press == other.press
            && self.enabled == other.enabled
            && self.loading == other.loading
            && self.icon_only == other.icon_only
            && self.focusless == other.focusless
            && self.style == other.style
    }
}

impl std::fmt::Debug for Button {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Button")
            .field("variant", &self.variant)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

impl View for Button {
    fn build(mut self) -> Element {
        let child = std::mem::replace(&mut self.child, Element::empty());
        let spinner = self
            .loading
            .then(|| spinner().extent(Length::px(self.size.icon_size())));
        Element::with_children(self, vec![spinner.build(), child])
    }
}

impl Styled for Button {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}
