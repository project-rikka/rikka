use rikka_theme::Theme;
use rikka_types::{Color, Key, Modifiers, RoundedRect};
use rikka_ui::text::{Label, TextStyled, text};
use rikka_ui::{
    Action, Align, Behavior, Element, Env, Event, EventCtx, Inset, IntoHandler, Justify, Layout,
    Length, PaintCtx, Quad, Style, Styled, View, Widget, motion,
};

use crate::control::ControlSize;
use crate::controls::press::Press;
use crate::controls::ripple::Ripple;

const BORDER: f32 = 1.0;
const RECORDING_BORDER: f32 = 2.0;

#[must_use]
pub struct ShortcutField {
    chord: Label,
    prompt: Label,
    recording: bool,
    warned: bool,
    size: ControlSize,
    on_record: Action,
    on_capture: Action<(Modifiers, Key)>,
    on_cancel: Action,
    style: Style,
}

pub fn shortcut_field(chord: impl Into<Label>) -> ShortcutField {
    ShortcutField {
        chord: chord.into(),
        prompt: Label::from("…"),
        recording: false,
        warned: false,
        size: ControlSize::default(),
        on_record: Action::default(),
        on_capture: Action::default(),
        on_cancel: Action::default(),
        style: Style::default(),
    }
}

impl ShortcutField {
    pub fn prompt(mut self, prompt: impl Into<Label>) -> Self {
        self.prompt = prompt.into();
        self
    }

    pub fn recording(mut self, recording: bool) -> Self {
        self.recording = recording;
        self
    }

    pub fn warned(mut self, warned: bool) -> Self {
        self.warned = warned;
        self
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn on_record<Marker>(mut self, on_record: impl IntoHandler<(), (), Marker>) -> Self {
        self.on_record = Action::new(on_record);
        self
    }

    pub fn on_capture<Marker>(
        mut self,
        on_capture: impl IntoHandler<(Modifiers, Key), (), Marker>,
    ) -> Self {
        self.on_capture = Action::new(on_capture);
        self
    }

    pub fn on_cancel<Marker>(mut self, on_cancel: impl IntoHandler<(), (), Marker>) -> Self {
        self.on_cancel = Action::new(on_cancel);
        self
    }

    fn is_active(&self) -> bool {
        self.on_record.is_set() || self.recording
    }

    fn border(&self, theme: &Theme) -> Color {
        let palette = theme.palette();
        match (self.recording, self.warned) {
            (true, _) => palette.accent,
            (false, true) => palette.warning,
            (false, false) => palette.outline,
        }
    }
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct ShortcutFieldState {
    press: Press,
    ripple: Ripple,
}

impl Widget for ShortcutField {
    type State = ShortcutFieldState;

    fn layout(&self) -> Layout {
        Layout::row()
            .align_items(Align::Center)
            .justify(Justify::Center)
            .padding(Inset::xy(self.size.padding_x(), 0.0))
            .size(Length::HUG, Length::px(self.size.height()))
            .fixed()
            .with(&self.style)
    }

    fn surroundings(&self, env: &mut Env) {
        let theme = rikka_ui::theme();
        let palette = theme.palette();
        env.text_color = match self.recording {
            true => palette.accent,
            false => palette.text,
        };
        env.font_size = self.size.font_size(&theme);
    }

    fn paint(&self, state: &mut ShortcutFieldState, ctx: &mut PaintCtx<'_>) {
        let radius = self.size.radius();
        let border = ctx.animate_curve("border", self.border(ctx.theme()), motion::COLOR);
        let width = match self.recording {
            true => RECORDING_BORDER,
            false => BORDER,
        };
        let rect = ctx.bounds();
        ctx.draw_quad(
            Quad::new(rect)
                .fill(ctx.theme().palette().surface)
                .radius(radius)
                .stroke(width, border),
        );
        state.ripple.paint(
            ctx,
            &state.press,
            RoundedRect::new(rect, radius),
            self.is_active(),
        );
    }

    fn event(&self, state: &mut ShortcutFieldState, event: &Event, ctx: &mut EventCtx<'_>) {
        if state.press.update(event, self.is_active(), ctx) && !self.recording {
            ctx.act(&self.on_record, ());
        }
        if !self.recording {
            return;
        }
        match event {
            Event::KeyPressed { key, modifiers, .. } => {
                ctx.set_handled();
                if key.is_modifier() || *key == Key::Unidentified {
                    return;
                }
                if *key == Key::Escape && *modifiers == Modifiers::NONE {
                    ctx.act(&self.on_cancel, ());
                    return;
                }
                ctx.act(&self.on_capture, (*modifiers, *key));
            }
            Event::KeyReleased { .. } => ctx.set_handled(),
            Event::FocusLost => {
                ctx.act(&self.on_cancel, ());
            }
            _ => {}
        }
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::TRACKS_HOVER
            | Behavior::FOCUSABLE.when(self.is_active())
            | Behavior::TAKES_FOCUS.when(self.recording)
            | Behavior::TAKES_TAB.when(self.recording)
            | Behavior::GRABS_KEYS.when(self.recording)
    }

    fn same(&self, other: &Self) -> bool {
        self.chord == other.chord
            && self.prompt == other.prompt
            && self.recording == other.recording
            && self.warned == other.warned
            && self.size == other.size
            && self.on_record == other.on_record
            && self.style == other.style
    }
}

impl std::fmt::Debug for ShortcutField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShortcutField")
            .field("chord", &self.chord.as_str())
            .field("recording", &self.recording)
            .finish_non_exhaustive()
    }
}

impl View for ShortcutField {
    fn build(self) -> Element {
        let shown = match self.recording {
            true => self.prompt.clone(),
            false => self.chord.clone(),
        };
        let label = text(shown).no_wrap().build();
        Element::with_children(self, vec![label])
    }
}

impl Styled for ShortcutField {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}
