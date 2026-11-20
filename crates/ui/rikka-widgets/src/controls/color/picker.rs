use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rikka_types::Color;
use rikka_ui::text::{TextDocument, TextStyled, text};
use rikka_ui::{
    Action, Align, Element, Handler, IntoHandler, Layout, Length, Space, StateAccess, Style,
    Styled, View, Widget,
};

use crate::blocks::flex::{column_of, row_of};
use crate::control::ControlSize;
use crate::controls::color::comparison::Comparison;
use crate::controls::color::plane::Plane;
use crate::controls::color::{Hsv, hex_of, parse_hex, same_color};
use crate::text::number_field::number_field;
use crate::text::text_field::text_field;

pub(crate) const WIDTH: f32 = 264.0;
const GAP: f32 = 10.0;
const CHANNEL_GAP: f32 = 6.0;
const HEX_LENGTH: usize = 9;
const CHANNEL_MAX: f32 = 255.0;

#[must_use]
#[derive(Widget)]
pub struct ColorPicker {
    #[debug]
    color: Color,
    #[debug]
    alpha: bool,
    size: ControlSize,
    on_change: Action<(Color,)>,
    on_commit: Action<(Color,)>,
    #[style]
    style: Style,
}

pub fn color_picker(color: Color) -> ColorPicker {
    ColorPicker {
        color,
        alpha: true,
        size: ControlSize::default(),
        on_change: Action::default(),
        on_commit: Action::default(),
        style: Style::default(),
    }
}

impl ColorPicker {
    pub fn alpha(mut self, alpha: bool) -> Self {
        self.alpha = alpha;
        self
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn on_change<Marker>(mut self, on_change: impl IntoHandler<(Color,), (), Marker>) -> Self {
        self.on_change = Action::new(on_change);
        self
    }

    pub fn on_commit<Marker>(mut self, on_commit: impl IntoHandler<(Color,), (), Marker>) -> Self {
        self.on_commit = Action::new(on_commit);
        self
    }

    fn shown(&self) -> Color {
        match self.alpha {
            true => self.color,
            false => self.color.with_alpha(1.0),
        }
    }

    fn emitter(&self, shared: &Rc<Shared>) -> Emitter {
        Emitter {
            shared: Rc::clone(shared),
            on_change: self.on_change.clone(),
            on_commit: self.on_commit.clone(),
        }
    }

    fn plane(&self, hsv: Hsv, emitter: Option<&Emitter>) -> Element {
        let on_pick = emitter.cloned().map_or_else(Action::default, |emitter| {
            Action::new(Handler::relaying(move |reach, (hsv, done): (Hsv, bool)| {
                emitter.emit(reach, hsv, done);
                Some(())
            }))
        });
        Plane {
            hsv,
            alpha: self.alpha,
            on_pick,
        }
        .build()
    }

    fn hex_row(&self, shared: &Rc<Shared>, emitter: Option<&Emitter>) -> Element {
        let comparison = Comparison {
            old: shared.original.get().unwrap_or(self.color),
            new: self.shown(),
            size: self.size,
            on_revert: emitter.cloned().map_or_else(Action::default, |emitter| {
                Action::new(Handler::relaying(move |reach, ()| {
                    let original = emitter.shared.original.get()?;
                    let hsv = emitter.shared.current().adopt(original);
                    emitter.emit(reach, hsv, true);
                    Some(())
                }))
            }),
        };
        let mut hex = text_field(&shared.hex)
            .control_size(self.size)
            .max_len(HEX_LENGTH)
            .enabled(emitter.is_some())
            .grow(1.0);
        if let Some(emitter) = emitter {
            let typing = Rc::clone(shared);
            let alpha = self.alpha;
            let committing = emitter.clone();
            let commit = Handler::relaying(move |reach, ()| {
                committing.commit_hex(reach, alpha);
                Some(())
            });
            hex = hex
                .on_change(Handler::relaying(move |_, ()| {
                    typing.hex_typing.set(true);
                    Some(())
                }))
                .on_submit(commit.clone())
                .on_blur(commit);
        }
        row_of([comparison.build(), hex.build()])
            .gap(Space::from(GAP))
            .align_items(Align::Center)
            .fill_width()
            .build()
    }

    fn channels(&self, emitter: Option<&Emitter>) -> Element {
        let color = self.shown();
        let count = match self.alpha {
            true => 4,
            false => 3,
        };
        let channels = ["R", "G", "B", "A"]
            .into_iter()
            .enumerate()
            .take(count)
            .map(|(channel, name)| {
                let value = (channel_of(color, channel) * CHANNEL_MAX).round();
                let mut field = number_field(value)
                    .range(0.0..=CHANNEL_MAX)
                    .control_size(self.size)
                    .enabled(emitter.is_some())
                    .fill_width();
                if let Some(emitter) = emitter.cloned() {
                    field = field.on_commit(Handler::relaying(move |reach, (typed,): (f32,)| {
                        let current = emitter.shared.current();
                        let color = with_channel(current.color(), channel, typed / CHANNEL_MAX);
                        emitter.emit(reach, current.adopt(color), true);
                        Some(())
                    }));
                }
                column_of([text(name).caption().muted().build(), field.build()])
                    .gap(Space::from(2.0))
                    .grow(1.0)
                    .shrink(1.0)
                    .min_width(Length::px(0.0))
                    .build()
            });
        row_of(channels)
            .gap(Space::from(CHANNEL_GAP))
            .fill_width()
            .build()
    }
}

fn channel_of(color: Color, channel: usize) -> f32 {
    match channel {
        0 => color.r,
        1 => color.g,
        2 => color.b,
        _ => color.a,
    }
}

fn with_channel(color: Color, channel: usize, value: f32) -> Color {
    let value = value.clamp(0.0, 1.0);
    match channel {
        0 => Color { r: value, ..color },
        1 => Color { g: value, ..color },
        2 => Color { b: value, ..color },
        _ => Color { a: value, ..color },
    }
}

#[derive(Default)]
struct Shared {
    hsv: Cell<Option<Hsv>>,
    original: Cell<Option<Color>>,
    hex: TextDocument,
    hex_shown: RefCell<String>,
    hex_typing: Cell<bool>,
}

impl Shared {
    fn current(&self) -> Hsv {
        self.hsv
            .get()
            .unwrap_or_else(|| Hsv::from_color(Color::BLACK))
    }

    fn adopt(&self, color: Color) -> Hsv {
        let hsv = match self.hsv.get() {
            Some(picker) => picker.adopt(color),
            None => Hsv::from_color(color),
        };
        self.hsv.set(Some(hsv));
        if self.original.get().is_none() {
            self.original.set(Some(color));
        }
        hsv
    }

    fn show_hex(&self, hex_text: String) {
        self.hex.reset(&hex_text);
        self.hex
            .edit(|content| content.place_caret(content.len(), false));
        *self.hex_shown.borrow_mut() = hex_text;
    }

    fn sync_hex(&self, color: Color, alpha: bool) {
        if self.hex_typing.get() {
            return;
        }
        let hex_text = hex_of(color, alpha);
        if *self.hex_shown.borrow() != hex_text {
            self.show_hex(hex_text);
        }
    }
}

#[derive(Clone)]
struct Emitter {
    shared: Rc<Shared>,
    on_change: Action<(Color,)>,
    on_commit: Action<(Color,)>,
}

impl Emitter {
    fn emit(&self, reach: &mut StateAccess<'_>, hsv: Hsv, done: bool) {
        let previous = self.shared.hsv.replace(Some(hsv));
        let color = hsv.color();
        let moved = previous.is_none_or(|previous| !same_color(previous.color(), color));
        if moved {
            self.on_change.call(reach, (color,));
        }
        if done {
            self.on_commit.call(reach, (color,));
        }
    }

    fn commit_hex(&self, reach: &mut StateAccess<'_>, alpha: bool) {
        if !self.shared.hex_typing.replace(false) {
            return;
        }
        let current = self.shared.current();
        let typed = self.shared.hex.text();
        match parse_hex(&typed, alpha, current.alpha) {
            Some(color) => {
                self.shared.show_hex(hex_of(color, alpha));
                self.emit(reach, current.adopt(color), true);
            }
            None => self.shared.show_hex(hex_of(current.color(), alpha)),
        }
    }
}

#[derive(Default)]
#[doc(hidden)]
pub struct ColorPickerState {
    shared: Rc<Shared>,
}

impl Widget for ColorPicker {
    type State = ColorPickerState;

    fn layout(&self) -> Layout {
        Layout::column()
            .gap_exact(GAP, GAP)
            .size(Length::px(WIDTH), Length::HUG)
            .with(&self.style)
    }

    fn build(&self, state: &mut ColorPickerState) -> Vec<Element> {
        let shared = &state.shared;
        let color = self.shown();
        let hsv = shared.adopt(color);
        shared.sync_hex(color, self.alpha);
        let emitter = self.on_change.is_set().then(|| self.emitter(shared));
        vec![
            self.plane(hsv, emitter.as_ref()),
            self.hex_row(shared, emitter.as_ref()),
            self.channels(emitter.as_ref()),
        ]
    }
}

impl std::fmt::Debug for ColorPickerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ColorPickerState")
            .field("hsv", &self.shared.hsv.get())
            .finish_non_exhaustive()
    }
}
