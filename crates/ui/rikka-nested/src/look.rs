use rikka_clipboard::Offer;
use rikka_types::{CursorShape, Point, Rect};

use crate::frame::Layer;

#[derive(Clone, Debug, Default)]
pub enum Cursor {
    #[default]
    Default,
    Shape(CursorShape),
    Hidden,
    Image {
        layer: Layer,
        hotspot: Point,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ContentPurpose {
    Normal,
    Alpha,
    Digits,
    Number,
    Phone,
    Url,
    Email,
    Name,
    Password,
    Pin,
    Date,
    Time,
    Datetime,
    Terminal,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ContentHints {
    pub completion: bool,
    pub spellcheck: bool,
    pub auto_capitalization: bool,
    pub lowercase: bool,
    pub uppercase: bool,
    pub titlecase: bool,
    pub hidden_text: bool,
    pub sensitive_data: bool,
    pub latin: bool,
    pub multiline: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Surrounding {
    pub text: String,
    pub cursor: usize,
    pub anchor: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextInput {
    pub purpose: ContentPurpose,
    pub hints: ContentHints,
    pub surrounding: Option<Surrounding>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Constraint {
    Locked,
    Confined,
}

#[derive(Clone, Debug)]
pub struct OutgoingDrag {
    pub offer: Option<Offer>,
    pub icon: Option<Layer>,
}

#[derive(Clone, Debug, Default)]
pub struct Look {
    pub attached: bool,
    pub title: Option<String>,
    pub cursor: Cursor,
    pub caret: Option<Rect>,
    pub text_input: Option<TextInput>,
    pub constraint: Option<Constraint>,
    pub inhibits_idle: bool,
    pub drag: Option<OutgoingDrag>,
    pub drop_accepted: bool,
}
