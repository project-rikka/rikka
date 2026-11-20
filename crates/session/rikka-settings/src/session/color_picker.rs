use std::collections::BTreeSet;

use rikka_types::Choice;

use crate::key::Key;
use crate::schema::Schema;
use crate::value::Value;

pub const DOMAIN: &str = "rikka.color-picker";

pub const ACTIVATION: &str = "activation";
pub const PRIMARY_CLICK: &str = "primary-click";
pub const MIDDLE_CLICK: &str = "middle-click";
pub const SECONDARY_CLICK: &str = "secondary-click";
pub const COPIED_FORMAT: &str = "copied-format";
pub const SHOW_NAME: &str = "show-color-name";
pub const FORMATS: &str = "formats";

pub const SHOW_NAME_DEFAULT: bool = false;

pub const HEX: &str = "HEX";

const BUILT_IN: [(&str, &str, bool); 16] = [
    (HEX, "%Rex%Grx%Blx", true),
    ("RGB", "rgb(%Re, %Gr, %Bl)", true),
    ("HSL", "hsl(%Hu, %Sl%, %Ll%)", true),
    ("HSV", "hsv(%Hu, %Sb%, %Va%)", false),
    ("CMYK", "cmyk(%Cy%, %Ma%, %Ye%, %Bk%)", false),
    ("HSB", "hsb(%Hu, %Sb%, %Br%)", false),
    ("HSI", "hsi(%Hu, %Si%, %In%)", false),
    ("HWB", "hwb(%Hu, %Wh%, %Bn%)", false),
    ("NCol", "%Hn, %Wh%, %Bn%", false),
    ("CIEXYZ", "XYZ(%Xv, %Yv, %Zv)", false),
    ("CIELAB", "CIELab(%Lc, %Ca, %Cb)", false),
    ("Oklab", "oklab(%Lo, %Oa, %Ob)", false),
    ("Oklch", "oklch(%Lo, %Oc, %Oh)", false),
    ("VEC4", "(%Reff, %Grff, %Blff, 1f)", false),
    ("Decimal", "%Dv", false),
    ("HEX Int", "0xFF%ReX%GrX%BlX", false),
];

pub const PICK: &str = "pick";
pub const EDITOR: &str = "editor";

pub const PICK_THEN_EDITOR: &str = "pick-then-editor";
pub const PICK_AND_CLOSE: &str = "pick-and-close";
pub const CLOSE: &str = "close";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Activation {
    #[default]
    Pick,
    Editor,
}

impl Choice for Activation {
    const ALL: &'static [Activation] = &[Activation::Pick, Activation::Editor];

    fn slug(self) -> &'static str {
        match self {
            Activation::Pick => PICK,
            Activation::Editor => EDITOR,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Click {
    #[default]
    PickThenEditor,
    PickAndClose,
    Close,
}

impl Choice for Click {
    const ALL: &'static [Click] = &[Click::PickThenEditor, Click::PickAndClose, Click::Close];

    fn slug(self) -> &'static str {
        match self {
            Click::PickThenEditor => PICK_THEN_EDITOR,
            Click::PickAndClose => PICK_AND_CLOSE,
            Click::Close => CLOSE,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Button {
    Primary,
    Middle,
    Secondary,
}

impl Button {
    pub const ALL: [Button; 3] = [Button::Primary, Button::Middle, Button::Secondary];

    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Button::Primary => PRIMARY_CLICK,
            Button::Middle => MIDDLE_CLICK,
            Button::Secondary => SECONDARY_CLICK,
        }
    }

    #[must_use]
    pub fn default_click(self) -> Click {
        match self {
            Button::Primary => Click::PickThenEditor,
            Button::Middle => Click::PickAndClose,
            Button::Secondary => Click::Close,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Format {
    pub name: String,
    pub format: String,
    #[serde(default = "shown_default")]
    pub shown: bool,
}

fn shown_default() -> bool {
    true
}

impl Format {
    #[must_use]
    pub fn new(name: impl Into<String>, format: impl Into<String>) -> Format {
        Format {
            name: name.into(),
            format: format.into(),
            shown: true,
        }
    }
}

#[must_use]
pub fn built_in_formats() -> Vec<Format> {
    BUILT_IN
        .into_iter()
        .map(|(name, format, shown)| Format {
            shown,
            ..Format::new(name, format)
        })
        .collect()
}

#[must_use]
pub fn formats_value(formats: &[Format]) -> Value {
    Value::from(&formats).unwrap_or(Value::List(Vec::new()))
}

fn built_in() -> Vec<Value> {
    match formats_value(&built_in_formats()) {
        Value::List(formats) => formats,
        _ => Vec::new(),
    }
}

#[must_use]
pub fn formats(value: &Value) -> Vec<Format> {
    value.get().unwrap_or_default()
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FormatError {
    #[error("color format has no name")]
    MissingName,
    #[error("duplicate color format name `{0}`")]
    DuplicateName(String),
}

pub fn validate_formats(formats: &[Format]) -> Result<(), FormatError> {
    let mut named = BTreeSet::new();
    for format in formats {
        let name = format.name.trim();
        if name.is_empty() {
            return Err(FormatError::MissingName);
        }
        if !named.insert(name) {
            return Err(FormatError::DuplicateName(name.to_owned()));
        }
    }
    Ok(())
}

#[must_use]
pub fn schema() -> Schema {
    let click = |button: Button, summary: &str| {
        Key::string(button.key(), button.default_click().slug())
            .choices(Click::ALL.iter().map(|click| click.slug()))
            .summary(summary)
    };
    Schema::new(DOMAIN)
        .summary("What the desktop's colour picker does, and how it writes the colours it picks")
        .key(Key::choice::<Activation>(ACTIVATION).summary(
            "What its shortcut brings up: the picker over the screen, or the editor of the \
             colours picked before",
        ))
        .key(click(
            Button::Primary,
            "What a left click does while picking: pick and open the editor, pick and close, \
             or close",
        ))
        .key(click(
            Button::Middle,
            "What a middle click does while picking",
        ))
        .key(click(
            Button::Secondary,
            "What a right click does while picking",
        ))
        .key(
            Key::string(COPIED_FORMAT, HEX).summary(
                "The name of the colour format a picked colour is copied to the clipboard in",
            ),
        )
        .key(
            Key::bool(SHOW_NAME, SHOW_NAME_DEFAULT).summary(
                "Whether the picker names the colour under the pointer, like `Light green`",
            ),
        )
        .key(
            Key::list(FORMATS, built_in())
                .summary(
                    "The colour formats the editor shows, in order: each a table of `name`, \
                     `format` (with `%Re`, `%Gr`, `%Bl` and the other parameters) and `shown`",
                )
                .checked_with(|value| {
                    let read: Vec<Format> = value
                        .get()
                        .map_err(|error| format!("not a list of colour formats: {error}"))?;
                    validate_formats(&read).map_err(|trouble| trouble.to_string())
                }),
        )
}
