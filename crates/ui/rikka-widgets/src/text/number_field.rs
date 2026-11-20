use std::ops::RangeInclusive;

use rikka_theme::Theme;
use rikka_types::{Key, Modifiers, Point, PointerButton, Rect};
use rikka_ui::text::{Label, TextDocument, TextRun};
use rikka_ui::{
    Action, Behavior, Event, EventCtx, IntoHandler, Layout, Length, PaintCtx, Style, Widget,
    measure_text,
};

use crate::control::ControlSize;
use crate::text::editing::{Editing, EditingState, Options, Room};
use crate::text::field::{self, FieldMood};

const LARGE_STEP: f32 = 10.0;
const WIDTH: f32 = 2.5;

#[must_use]
#[derive(Widget)]
pub struct NumberField {
    #[debug]
    value: f32,
    range: RangeInclusive<f32>,
    step: f32,
    decimals: u8,
    unit: Option<Label>,
    size: ControlSize,
    enabled: bool,
    on_commit: Action<(f32,)>,
    on_submit: Action<()>,
    #[style]
    style: Style,
}

pub fn number_field(value: f32) -> NumberField {
    NumberField {
        value,
        range: f32::MIN..=f32::MAX,
        step: 1.0,
        decimals: 0,
        unit: None,
        size: ControlSize::default(),
        enabled: true,
        on_commit: Action::default(),
        on_submit: Action::default(),
        style: Style::default(),
    }
}

impl NumberField {
    pub fn range(mut self, range: RangeInclusive<f32>) -> Self {
        self.range = range;
        self
    }

    pub fn step(mut self, step: f32) -> Self {
        self.step = step.abs();
        self
    }

    pub fn decimals(mut self, decimals: u8) -> Self {
        self.decimals = decimals;
        self
    }

    pub fn unit(mut self, unit: impl Into<Label>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn on_commit<Marker>(mut self, on_commit: impl IntoHandler<(f32,), (), Marker>) -> Self {
        self.on_commit = Action::new(on_commit);
        self
    }

    pub fn on_submit<Marker>(mut self, on_submit: impl IntoHandler<(), (), Marker>) -> Self {
        self.on_submit = Action::new(on_submit);
        self
    }

    fn is_active(&self) -> bool {
        self.enabled && self.on_commit.active(true)
    }

    fn settled(&self, value: f32) -> f32 {
        let scale = 10f32.powi(i32::from(self.decimals.min(9)));
        let rounded = (value * scale).round() / scale;
        rounded.max(*self.range.start()).min(*self.range.end())
    }

    fn format(&self, value: f32) -> String {
        let formatted = format!("{:.*}", usize::from(self.decimals), value);
        match formatted
            .trim_start_matches('-')
            .chars()
            .all(|c| c == '0' || c == '.')
        {
            true => formatted.trim_start_matches('-').to_owned(),
            false => formatted,
        }
    }

    fn parse(&self, typed: &str) -> Option<f32> {
        let mut typed = typed.trim();
        if let Some(unit) = &self.unit {
            let unit = unit.as_str().trim();
            if !unit.is_empty()
                && typed.len() >= unit.len()
                && typed.is_char_boundary(typed.len() - unit.len())
                && typed[typed.len() - unit.len()..].eq_ignore_ascii_case(unit)
            {
                typed = typed[..typed.len() - unit.len()].trim_end();
            }
        }
        let decimal = match typed.contains('.') {
            true => typed.to_owned(),
            false => typed.replacen(',', ".", 1),
        };
        decimal
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite())
            .map(|value| self.settled(value))
    }

    fn editing(&self, document: &TextDocument) -> Editing {
        let mut options = Options::field();
        options.editable = self.is_active();
        Editing::new(document, options)
    }

    fn padding(&self) -> f32 {
        self.size.text_inset()
    }

    fn font_size(&self, theme: &Theme) -> f32 {
        self.size.font_size(theme)
    }

    fn unit_run(&self, theme: &Theme) -> Option<TextRun> {
        let unit = self.unit.clone()?;
        Some(TextRun::new(unit, self.font_size(theme)).no_wrap())
    }

    fn sync(&self, state: &mut NumberFieldState) {
        if state.typing() {
            return;
        }
        match &mut state.shown {
            Some(shown) if shown.value.to_bits() == self.value.to_bits() => {}
            Some(shown) if shown.text == self.format(self.value) => shown.value = self.value,
            _ => state.show(self.value, self.format(self.value)),
        }
    }

    fn current(&self, state: &NumberFieldState) -> f32 {
        self.parse(&state.document.text())
            .unwrap_or_else(|| self.settled(self.value))
    }

    fn report(&self, value: f32, ctx: &mut EventCtx<'_>) {
        if value != self.value {
            ctx.act(&self.on_commit, (value,));
        }
    }

    fn commit(&self, state: &mut NumberFieldState, ctx: &mut EventCtx<'_>) {
        if !state.typing() {
            return;
        }
        let parsed = self.parse(&state.document.text());
        let value = parsed.unwrap_or(self.value);
        state.show(value, self.format(value));
        ctx.request_paint();
        if parsed.is_some() {
            self.report(value, ctx);
        }
    }

    fn revert(&self, state: &mut NumberFieldState, ctx: &mut EventCtx<'_>) {
        state.show(self.value, self.format(self.value));
        state.document.select_all();
        ctx.request_paint();
    }

    fn nudge(&self, state: &mut NumberFieldState, steps: f32, ctx: &mut EventCtx<'_>) {
        let value = self.settled(self.current(state) + steps * self.step);
        state.show(value, self.format(value));
        ctx.request_paint();
        self.report(value, ctx);
    }

    fn keyed(
        &self,
        state: &mut NumberFieldState,
        key: Key,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        let large = match modifiers.shift {
            true => LARGE_STEP,
            false => 1.0,
        };
        match key {
            Key::ArrowUp => self.nudge(state, large, ctx),
            Key::ArrowDown => self.nudge(state, -large, ctx),
            Key::PageUp => self.nudge(state, LARGE_STEP, ctx),
            Key::PageDown => self.nudge(state, -LARGE_STEP, ctx),
            Key::Enter => {
                self.commit(state, ctx);
                state.document.select_all();
                ctx.act(&self.on_submit, ());
            }
            Key::Escape if state.typing() => self.revert(state, ctx),
            _ => return false,
        }
        ctx.set_handled();
        true
    }

    fn wheeled(
        &self,
        state: &mut NumberFieldState,
        up: f32,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) {
        ctx.set_handled();
        state.wheel += up;
        let notches = state.wheel.trunc();
        if notches == 0.0 {
            return;
        }
        state.wheel -= notches;
        let large = match modifiers.shift {
            true => LARGE_STEP,
            false => 1.0,
        };
        self.nudge(state, notches * large, ctx);
    }
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct NumberFieldState {
    document: TextDocument,
    editing: EditingState,
    shown: Option<DisplayedValue>,
    pressing: bool,
    wheel: f32,
}

#[derive(Debug)]
struct DisplayedValue {
    value: f32,
    text: String,
    revision: u64,
}

impl NumberFieldState {
    fn show(&mut self, value: f32, text: String) {
        self.document.reset(&text);
        self.document
            .edit(|content| content.place_caret(content.len(), false));
        self.shown = Some(DisplayedValue {
            value,
            text,
            revision: self.document.revision(),
        });
    }

    fn typing(&self) -> bool {
        self.shown
            .as_ref()
            .is_some_and(|shown| shown.revision != self.document.revision())
    }
}

impl Widget for NumberField {
    type State = NumberFieldState;

    fn layout(&self) -> Layout {
        Layout::row()
            .size(
                Length::px((self.size.height() * WIDTH).round()),
                Length::px(self.size.height()),
            )
            .fixed()
            .with(&self.style)
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::CLIPS | Behavior::TRACKS_HOVER | Behavior::FOCUSABLE.when(self.is_active())
    }

    fn paint(&self, state: &mut NumberFieldState, ctx: &mut PaintCtx<'_>) {
        self.sync(state);
        let palette = ctx.theme().palette();
        let focused = ctx.focused();
        let hovered = ctx.hovered();
        let bounds = ctx.bounds();
        let active = self.is_active();

        let (hint, ink, muted) = {
            let theme = ctx.theme();
            (
                theme.disabled(palette.text_muted),
                theme.dim(palette.text, !active),
                theme.dim(palette.text_muted, !active),
            )
        };
        let mood = FieldMood {
            enabled: active,
            attentive: focused,
            hovered,
            invalid: false,
        };
        field::paint_frame(ctx, bounds, self.size.radius(), mood);

        let padding = self.padding();
        let mut right = padding;
        if let Some(run) = self.unit_run(ctx.theme()) {
            let width = measure_text(&run, None).width;
            let gap = self.size.gap() / 2.0;
            let x = bounds.max_x() - padding - width;
            ctx.draw_text_line(bounds, x, width, run, muted);
            right += width + gap;
        }

        let template = TextRun::new("", self.font_size(ctx.theme())).no_wrap();
        let room = Room {
            bounds,
            left: padding,
            right,
            top: field::text_top(ctx, bounds, template.line_height),
            ink,
            hint,
        };
        let editing = self.editing(&state.document);
        editing.paint(&mut state.editing, ctx, room, &template);
    }

    fn event(&self, state: &mut NumberFieldState, event: &Event, ctx: &mut EventCtx<'_>) {
        if !self.is_active() {
            return;
        }
        if state.editing.menu.event(event, ctx) {
            return;
        }
        self.sync(state);
        match *event {
            Event::KeyPressed { key, modifiers, .. }
                if ctx.focused() && self.keyed(state, key, modifiers, ctx) =>
            {
                return;
            }
            Event::Scroll {
                delta, modifiers, ..
            } if ctx.focused() => {
                self.wheeled(state, delta.up(), modifiers, ctx);
                return;
            }
            Event::PointerPressed {
                button: PointerButton::Primary | PointerButton::Secondary,
                ..
            } => state.pressing = true,
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } => state.pressing = false,
            Event::FocusGained => {
                if !state.pressing {
                    state.document.select_all();
                    ctx.request_paint();
                }
            }
            Event::FocusLost => {
                state.pressing = false;
                state.wheel = 0.0;
                self.commit(state, ctx);
            }
            _ => {}
        }
        self.editing(&state.document)
            .event(&mut state.editing, event, ctx);
    }

    fn caret(&self, state: &NumberFieldState) -> Option<Rect> {
        state.editing.caret_within()
    }

    fn relocated(&self, state: &mut NumberFieldState, by: Point) {
        state.editing.relocated(by);
    }

    fn same(&self, other: &Self) -> bool {
        self.value == other.value
            && self.range == other.range
            && self.step == other.step
            && self.decimals == other.decimals
            && self.unit == other.unit
            && self.size == other.size
            && self.enabled == other.enabled
            && self.on_commit == other.on_commit
            && self.on_submit == other.on_submit
            && self.style == other.style
    }
}
