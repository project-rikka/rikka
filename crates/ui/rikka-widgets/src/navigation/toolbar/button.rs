use rikka_input::Chord;
use rikka_symbols::MaterialSymbol;
use rikka_ui::text::Label;
use rikka_ui::{Action, Element, IntoHandler, Length, Tooltip, View};

use crate::control::ControlSize;
use crate::controls::button::{ButtonVariant, button};
use crate::media::icon::icon;
use crate::navigation::menu::{MenuItem, menu_item};
use crate::navigation::toolbar::item::{ToolItem, tool_item};

#[must_use]
#[derive(Debug)]
pub struct ToolButton {
    symbol: MaterialSymbol,
    label: Label,
    shortcut: Option<Chord>,
    press: Action,
    enabled: bool,
    checked: bool,
    variant: ButtonVariant,
    size: ControlSize,
}

pub fn tool_button(symbol: MaterialSymbol, label: impl Into<Label>) -> ToolButton {
    ToolButton {
        symbol,
        label: label.into(),
        shortcut: None,
        press: Action::default(),
        enabled: true,
        checked: false,
        variant: ButtonVariant::Ghost,
        size: ControlSize::Small,
    }
}

impl ToolButton {
    pub fn on_press<Marker>(mut self, press: impl IntoHandler<(), (), Marker>) -> Self {
        self.press = Action::new(press);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn shortcut(mut self, chord: impl Into<Option<Chord>>) -> Self {
        self.shortcut = chord.into();
        self
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    fn glyph(&self) -> f32 {
        match self.size {
            ControlSize::Small => 18.0,
            ControlSize::Medium | ControlSize::Large => 24.0,
        }
    }

    fn face(&self) -> ButtonVariant {
        match (self.checked, self.variant) {
            (true, ButtonVariant::Ghost) => ButtonVariant::Secondary,
            (_, variant) => variant,
        }
    }

    fn entry(&self) -> MenuItem {
        let entry = menu_item(self.label.clone())
            .icon(self.symbol)
            .enabled(self.enabled)
            .selected(self.checked);
        let entry = match self.shortcut {
            Some(chord) => entry.shortcut(chord.to_string()),
            None => entry,
        };
        match self.press.handler() {
            Some(handler) => entry.on_press(handler.clone()),
            None => entry,
        }
    }
}

impl View for ToolButton {
    fn build(self) -> Element {
        let face = button(icon(self.symbol).extent(Length::px(self.glyph())))
            .icon_only()
            .focusless()
            .control_size(self.size)
            .variant(self.face())
            .enabled(self.enabled);
        let face = match self.press.handler() {
            Some(handler) => face.on_press(handler.clone()),
            None => face,
        };
        face.tooltip(Tooltip::new(self.label).shortcut(self.shortcut))
            .build()
    }
}

impl From<ToolButton> for ToolItem {
    fn from(tool: ToolButton) -> Self {
        let entry = tool.entry();
        tool_item(tool).overflow([entry])
    }
}
