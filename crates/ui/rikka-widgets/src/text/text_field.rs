use rikka_theme::Theme;
use rikka_types::{Key, Modifiers, Point, Rect};
use rikka_ui::text::{Label, TextDocument, TextRun};
use rikka_ui::{
    Action, Align, Behavior, Command, Element, Env, Event, EventCtx, Inset, IntoHandler, Justify,
    Layout, Length, PaintCtx, Style, Styled, View, Widget,
};

use crate::control::ControlSize;
use crate::navigation::menu::MenuItem;
use crate::text::editing::{Editing, EditingState, Options, Room};
use crate::text::field::{self, FieldMood};

#[must_use]
pub struct TextField {
    editing: Editing,
    trailing: Option<Element>,
    size: ControlSize,
    error: bool,
    enabled: bool,
    autofocus: bool,
    claims_focus: bool,
    plain: bool,
    font_size: Option<f32>,
    style: Style,
}

pub fn text_field(document: &TextDocument) -> TextField {
    TextField {
        editing: Editing::new(document, Options::field()),
        trailing: None,
        size: ControlSize::default(),
        error: false,
        enabled: true,
        autofocus: false,
        claims_focus: false,
        plain: false,
        font_size: None,
        style: Style::default(),
    }
}

impl TextField {
    pub fn placeholder(mut self, placeholder: impl Into<Label>) -> Self {
        self.editing.placeholder = Some(placeholder.into());
        self
    }

    pub fn trailing(mut self, trailing: impl View) -> Self {
        self.trailing = Some(trailing.build());
        self
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn max_len(mut self, max: usize) -> Self {
        self.editing.options.max_len = Some(max);
        self
    }

    pub fn on_change<Marker>(mut self, on_change: impl IntoHandler<(), (), Marker>) -> Self {
        self.editing.on_change = Action::new(on_change);
        self
    }

    pub fn on_submit<Marker>(mut self, on_submit: impl IntoHandler<(), (), Marker>) -> Self {
        self.editing.on_submit = Action::new(on_submit);
        self
    }

    pub fn on_blur<Marker>(mut self, on_blur: impl IntoHandler<(), (), Marker>) -> Self {
        self.editing.on_blur = Action::new(on_blur);
        self
    }

    pub fn on_key<Marker>(
        mut self,
        on_key: impl IntoHandler<(Key, Modifiers), bool, Marker>,
    ) -> Self {
        self.editing.on_key = Action::new(on_key);
        self
    }

    pub fn on_command<Marker>(
        mut self,
        on_command: impl IntoHandler<(Command,), bool, Marker>,
    ) -> Self {
        self.editing.on_command = Action::new(on_command);
        self
    }

    pub fn context_items<Marker>(
        mut self,
        items: impl IntoHandler<(), Vec<MenuItem>, Marker>,
    ) -> Self {
        self.editing.context_items = Action::new(items);
        self
    }

    pub fn error(mut self, error: bool) -> Self {
        self.error = error;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self.editing.options.editable = enabled;
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = Some(size);
        self
    }

    pub fn plain(mut self, plain: bool) -> Self {
        self.plain = plain;
        self
    }

    pub fn secret(mut self, secret: bool) -> Self {
        self.editing.options.secret = secret;
        self
    }

    pub fn leaves_arrows(mut self, leaves_arrows: bool) -> Self {
        self.editing.options.leaves_arrows = leaves_arrows;
        self
    }

    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }

    pub fn claims_focus(mut self, claims_focus: bool) -> Self {
        self.claims_focus = claims_focus;
        self
    }

    fn padding(&self) -> f32 {
        self.size.text_inset()
    }

    fn text_size(&self, theme: &Theme) -> f32 {
        self.font_size.unwrap_or_else(|| self.size.font_size(theme))
    }

    fn template(&self, ctx: &mut PaintCtx<'_>) -> TextRun {
        let theme = ctx.theme();
        TextRun::new("", self.text_size(theme)).no_wrap()
    }

    fn right(&self, bounds: Rect, trailing: Option<Rect>) -> f32 {
        match trailing {
            Some(rect) => (bounds.max_x() - rect.min_x() + self.size.gap()).max(0.0),
            None => self.padding(),
        }
    }
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct TextFieldState {
    editing: EditingState,
}

impl Widget for TextField {
    type State = TextFieldState;

    fn layout(&self) -> Layout {
        Layout::row()
            .align_items(Align::Center)
            .justify(Justify::End)
            .padding(Inset::xy(self.padding(), 0.0))
            .size(Length::FILL, Length::px(self.size.height()))
            .fixed()
            .with(&self.style)
    }

    fn surroundings(&self, env: &mut Env) {
        let theme = rikka_ui::theme();
        env.font_size = self.text_size(&theme);
        if !self.enabled {
            env.text_color = theme.disabled(theme.palette().text);
        }
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::CLIPS
            | Behavior::TRACKS_HOVER
            | Behavior::FOCUSABLE.when(self.enabled)
            | Behavior::AUTOFOCUS.when(self.autofocus)
            | Behavior::TAKES_FOCUS.when(self.claims_focus)
    }

    fn paint(&self, state: &mut TextFieldState, ctx: &mut PaintCtx<'_>) {
        let palette = ctx.theme().palette();
        let focused = ctx.focused();
        let hovered = ctx.hovered();
        let bounds = ctx.bounds();

        let radius = self.size.radius();
        let hint = ctx.theme().disabled(palette.text_muted);
        if !self.plain {
            let mood = FieldMood {
                enabled: self.enabled,
                attentive: focused,
                hovered,
                invalid: self.error,
            };
            field::paint_frame(ctx, bounds, radius, mood);
        }

        let template = self.template(ctx);
        let room = Room {
            bounds,
            left: self.padding(),
            right: self.right(bounds, ctx.child_rect(0)),
            top: field::text_top(ctx, bounds, template.line_height),
            ink: ctx.env().text_color,
            hint,
        };
        self.editing.paint(&mut state.editing, ctx, room, &template);
    }

    fn event(&self, state: &mut TextFieldState, event: &Event, ctx: &mut EventCtx<'_>) {
        if !self.enabled {
            return;
        }
        self.editing.event(&mut state.editing, event, ctx);
    }

    fn caret(&self, state: &TextFieldState) -> Option<Rect> {
        self.editing
            .composes()
            .then(|| state.editing.caret_within())
            .flatten()
    }

    fn relocated(&self, state: &mut TextFieldState, by: Point) {
        state.editing.relocated(by);
    }

    fn same(&self, other: &Self) -> bool {
        self.editing.looks_like(&other.editing)
            && self.size == other.size
            && self.error == other.error
            && self.enabled == other.enabled
            && self.autofocus == other.autofocus
            && self.claims_focus == other.claims_focus
            && self.plain == other.plain
            && self.font_size == other.font_size
            && self.style == other.style
    }
}

impl std::fmt::Debug for TextField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextField")
            .field("document", &self.editing.document)
            .field("secret", &self.editing.is_secret())
            .finish_non_exhaustive()
    }
}

impl View for TextField {
    fn build(mut self) -> Element {
        let children = self.trailing.take().into_iter().collect();
        Element::with_children(self, children)
    }
}

impl Styled for TextField {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}
