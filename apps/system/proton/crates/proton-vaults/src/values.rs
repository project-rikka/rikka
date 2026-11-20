use rikka_proton_pass::{ExtraField, ExtraValue};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Card {
    pub holder: String,
    pub number: String,
    pub verification: String,
    pub expiration: String,
    pub pin: String,
    pub kind: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Wifi {
    pub ssid: String,
    pub password: String,
    pub security: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ssh {
    pub public: String,
    pub private: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CustomField {
    pub name: String,
    pub value: String,
    pub hidden: bool,
}

impl CustomField {
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>, hidden: bool) -> CustomField {
        CustomField {
            name: name.into(),
            value: value.into(),
            hidden,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.name.trim().is_empty() && self.value.trim().is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Text(String),
    Hidden(String),
    Code(String),
    Timestamp(i64),
}

impl Value {
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Value::Text(text) | Value::Hidden(text) | Value::Code(text) => text.clone(),
            Value::Timestamp(at) => at.to_string(),
        }
    }

    #[must_use]
    pub fn is_hidden(&self) -> bool {
        matches!(self, Value::Hidden(_))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub value: Value,
}

impl Field {
    #[must_use]
    pub fn new(name: impl Into<String>, value: Value) -> Field {
        Field {
            name: name.into(),
            value,
        }
    }

    #[must_use]
    pub fn from_extra(field: &ExtraField) -> Field {
        let value = match field.value() {
            Some(ExtraValue::Text(text)) => Value::Text(text.to_owned()),
            Some(ExtraValue::Hidden(text)) => Value::Hidden(text.to_owned()),
            Some(ExtraValue::Totp(text)) => Value::Code(text.to_owned()),
            Some(ExtraValue::Timestamp(at)) => Value::Timestamp(at.seconds),
            None => Value::Text(String::new()),
        };

        Field::new(field.name(), value)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Group {
    pub name: String,
    pub fields: Vec<CustomField>,
}

impl Group {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.name.trim().is_empty() && self.fields.iter().all(CustomField::is_empty)
    }
}
