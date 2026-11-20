use rikka_symbols::MaterialSymbol;
use rikka_ui::{
    Action, Behavior, Element, Env, Event, EventCtx, IntoHandler, Layout, PaintCtx, Style, Styled,
    View, Widget, motion,
};

use crate::control::ControlSize;
use crate::controls::choice;
use crate::controls::mark::Mark;
use crate::controls::press::Press;
use crate::controls::ripple::Ripple;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CheckState {
    #[default]
    Unchecked,
    Checked,
    Indeterminate,
}

impl CheckState {
    pub(crate) fn is_marked(self) -> bool {
        self != Self::Unchecked
    }

    #[must_use]
    pub fn toggled(self) -> bool {
        !matches!(self, Self::Checked)
    }

    pub(crate) fn glyph(self) -> Option<MaterialSymbol> {
        match self {
            Self::Unchecked => None,
            Self::Checked => Some(MaterialSymbol::CHECK),
            Self::Indeterminate => Some(MaterialSymbol::REMOVE),
        }
    }
}

impl From<bool> for CheckState {
    fn from(checked: bool) -> Self {
        if checked {
            Self::Checked
        } else {
            Self::Unchecked
        }
    }
}

#[must_use]
pub struct Checkbox {
    state: CheckState,
    label: Option<Element>,
    size: ControlSize,
    on_toggle: Action<(bool,)>,
    enabled: bool,
    style: Style,
}

pub fn checkbox(state: impl Into<CheckState>) -> Checkbox {
    Checkbox {
        state: state.into(),
        label: None,
        size: ControlSize::default(),
        on_toggle: Action::default(),
        enabled: true,
        style: Style::default(),
    }
}

impl Checkbox {
    pub fn label(mut self, label: impl View) -> Self {
        self.label = Some(label.build());
        self
    }

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
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct CheckboxState {
    press: Press,
    ripple: Ripple,
    mark: Option<MaterialSymbol>,
}

impl Widget for Checkbox {
    type State = CheckboxState;

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

    fn paint(&self, state: &mut CheckboxState, ctx: &mut PaintCtx<'_>) {
        let active = self.is_active();
        let box_rect = choice::paint_row(ctx, &mut state.ripple, &state.press, self.size, active);

        let mut mark = Mark::settled(self.state, ctx.theme(), box_rect.width(), self.enabled);
        let (fill, radius) = (mark.fill, mark.radius);
        mark.fill = ctx.animate_curve("fill", fill, motion::COLOR);
        mark.radius = ctx.animate_curve("radius", radius, motion::MORPH);

        if let Some(glyph) = mark.glyph {
            state.mark = Some(glyph);
        }
        mark.glyph = state.mark;
        let grown = if mark.marked { 1.0 } else { 0.0 };
        mark.grown = ctx.animate_curve("glyph", grown, motion::TOGGLE);
        mark.paint(ctx, box_rect);
    }

    fn event(&self, state: &mut CheckboxState, event: &Event, ctx: &mut EventCtx<'_>) {
        if state.press.update(event, self.is_active(), ctx) {
            ctx.act(&self.on_toggle, (self.state.toggled(),));
        }
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::TRACKS_HOVER | Behavior::FOCUSABLE.when(self.on_toggle.active(self.enabled))
    }

    fn same(&self, other: &Self) -> bool {
        self.state == other.state
            && self.size == other.size
            && self.on_toggle == other.on_toggle
            && self.enabled == other.enabled
            && self.style == other.style
    }
}

impl std::fmt::Debug for Checkbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Checkbox")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl View for Checkbox {
    fn build(mut self) -> Element {
        let children = self.label.take().into_iter().collect();
        Element::with_children(self, children)
    }
}

impl Styled for Checkbox {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}
