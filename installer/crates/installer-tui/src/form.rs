use std::borrow::Cow;

use installer_core::{Disk, Secret, byte_index};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldId {
    Locale,
    EnglishMessages,
    Keyboard,
    Timezone,
    Disk,
    Filesystem,
    Swap,
    Encrypt,
    Passphrase,
    PassphraseConfirm,
    Hostname,
    Username,
    FullName,
    Password,
    PasswordConfirm,
    SetRootPassword,
    RootPassword,
    RootPasswordConfirm,
    Channel,
    Kernel,
    AllowUnfree,
    OpenSsh,
    SshPasswordAuth,
    Nvidia,
    SudoRs,
    App(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub label: String,
    pub note: String,
}

impl Choice {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            note: String::new(),
        }
    }

    #[must_use]
    pub fn with_note(label: impl Into<String>, note: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            note: note.into(),
        }
    }
}

#[derive(Debug)]
pub enum TextValue {
    Plain(String),
    Hidden(Secret),
}

impl TextValue {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Plain(text) => text,
            Self::Hidden(secret) => secret.as_str(),
        }
    }

    #[must_use]
    pub fn chars(&self) -> usize {
        match self {
            Self::Plain(text) => text.chars().count(),
            Self::Hidden(secret) => secret.chars(),
        }
    }

    fn insert(&mut self, at: usize, ch: char) -> bool {
        match self {
            Self::Plain(text) => {
                text.insert(byte_index(text, at), ch);
                true
            }
            Self::Hidden(secret) => secret.insert(at, ch),
        }
    }

    fn remove_before(&mut self, at: usize) {
        match self {
            Self::Plain(text) => {
                if at > 0 {
                    text.remove(byte_index(text, at - 1));
                }
            }
            Self::Hidden(secret) => secret.remove_before(at),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Warning,
    Danger,
}

#[derive(Debug)]
pub enum Field {
    Text {
        id: FieldId,
        label: &'static str,
        placeholder: &'static str,
        value: TextValue,
        cursor: usize,
    },
    Choice {
        id: FieldId,
        label: &'static str,
        options: Vec<Choice>,
        index: usize,
    },
    Toggle {
        id: FieldId,
        label: Cow<'static, str>,
        on: bool,
    },
    Picker {
        id: FieldId,
        label: &'static str,
        options: Vec<Choice>,
        index: usize,
    },
    Disks {
        id: FieldId,
        label: &'static str,
        disks: Vec<Disk>,
        index: usize,
    },
}

impl Field {
    #[must_use]
    pub fn text(id: FieldId, label: &'static str, value: impl Into<String>) -> Self {
        let value = TextValue::Plain(value.into());
        Self::Text {
            id,
            label,
            placeholder: "",
            cursor: value.chars(),
            value,
        }
    }

    #[must_use]
    pub fn secret(id: FieldId, label: &'static str, value: Secret) -> Self {
        let cursor = value.chars();
        Self::Text {
            id,
            label,
            placeholder: "",
            value: TextValue::Hidden(value),
            cursor,
        }
    }

    #[must_use]
    pub fn choice(id: FieldId, label: &'static str, options: Vec<Choice>, index: usize) -> Self {
        Self::Choice {
            id,
            label,
            index: index.min(options.len().saturating_sub(1)),
            options,
        }
    }

    #[must_use]
    pub fn toggle(id: FieldId, label: impl Into<Cow<'static, str>>, on: bool) -> Self {
        Self::Toggle {
            id,
            label: label.into(),
            on,
        }
    }

    #[must_use]
    pub fn picker(id: FieldId, label: &'static str, options: Vec<Choice>, index: usize) -> Self {
        Self::Picker {
            id,
            label,
            index: index.min(options.len().saturating_sub(1)),
            options,
        }
    }

    #[must_use]
    pub fn disks(id: FieldId, label: &'static str, disks: Vec<Disk>, index: usize) -> Self {
        Self::Disks {
            id,
            label,
            index: index.min(disks.len().saturating_sub(1)),
            disks,
        }
    }

    #[must_use]
    pub fn placeholder(mut self, text: &'static str) -> Self {
        if let Self::Text { placeholder, .. } = &mut self {
            *placeholder = text;
        }
        self
    }

    #[must_use]
    pub fn id(&self) -> FieldId {
        match self {
            Self::Text { id, .. }
            | Self::Choice { id, .. }
            | Self::Toggle { id, .. }
            | Self::Picker { id, .. }
            | Self::Disks { id, .. } => *id,
        }
    }

    pub fn insert(&mut self, ch: char) {
        if let Self::Text { value, cursor, .. } = self
            && value.insert(*cursor, ch)
        {
            *cursor += 1;
        }
    }

    pub fn backspace(&mut self) {
        if let Self::Text { value, cursor, .. } = self {
            value.remove_before(*cursor);
            *cursor = cursor.saturating_sub(1);
        }
    }

    pub fn cycle(&mut self, delta: isize) {
        match self {
            Self::Text { cursor, value, .. } => {
                *cursor = cursor.saturating_add_signed(delta).min(value.chars());
            }
            Self::Choice { index, options, .. } | Self::Picker { index, options, .. } => {
                *index = wrap(*index, delta, options.len());
            }
            Self::Disks { index, disks, .. } => *index = wrap(*index, delta, disks.len()),
            Self::Toggle { on, .. } => *on = !*on,
        }
    }

    #[must_use]
    pub fn is_list(&self) -> bool {
        matches!(self, Self::Disks { .. })
    }

    pub fn activate(&mut self) {
        if let Self::Toggle { on, .. } = self {
            *on = !*on;
        }
    }

    pub fn go_home(&mut self) {
        if let Self::Text { cursor, .. } = self {
            *cursor = 0;
        }
    }

    pub fn go_end(&mut self) {
        if let Self::Text { cursor, value, .. } = self {
            *cursor = value.chars();
        }
    }
}

fn wrap(index: usize, delta: isize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let len = len as isize;
    ((index as isize + delta).rem_euclid(len)) as usize
}

#[derive(Debug)]
pub struct Form {
    fields: Vec<Field>,
    focus: usize,
}

impl Form {
    #[must_use]
    pub fn new(fields: Vec<Field>) -> Self {
        Self { fields, focus: 0 }
    }

    #[must_use]
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    #[must_use]
    pub fn focus(&self) -> usize {
        self.focus
    }

    #[must_use]
    pub fn on_last(&self) -> bool {
        self.focus + 1 >= self.fields.len()
    }

    #[must_use]
    pub fn focused(&self) -> Option<&Field> {
        self.fields.get(self.focus)
    }

    pub fn focused_mut(&mut self) -> Option<&mut Field> {
        self.fields.get_mut(self.focus)
    }

    pub fn step_focus(&mut self, delta: isize) {
        self.focus = wrap(self.focus, delta, self.fields.len());
    }

    pub fn set_focus(&mut self, index: usize) {
        self.focus = index.min(self.fields.len().saturating_sub(1));
    }

    #[must_use]
    pub fn shape(&self) -> Vec<FieldId> {
        self.fields.iter().map(Field::id).collect()
    }

    #[must_use]
    pub fn get(&self, id: FieldId) -> Option<&Field> {
        self.fields.iter().find(|field| field.id() == id)
    }

    pub fn get_mut(&mut self, id: FieldId) -> Option<&mut Field> {
        self.fields.iter_mut().find(|field| field.id() == id)
    }

    #[must_use]
    pub fn value(&self, id: FieldId) -> &str {
        match self.get(id) {
            Some(Field::Text { value, .. }) => value.as_str(),
            _ => "",
        }
    }

    #[must_use]
    pub fn secret(&self, id: FieldId) -> Secret {
        match self.get(id) {
            Some(Field::Text {
                value: TextValue::Hidden(secret),
                ..
            }) => secret.clone(),
            _ => Secret::new(),
        }
    }

    #[must_use]
    pub fn index(&self, id: FieldId) -> usize {
        match self.get(id) {
            Some(
                Field::Choice { index, .. }
                | Field::Picker { index, .. }
                | Field::Disks { index, .. },
            ) => *index,
            _ => 0,
        }
    }

    #[must_use]
    pub fn is_checked(&self, id: FieldId) -> bool {
        matches!(self.get(id), Some(Field::Toggle { on: true, .. }))
    }

    #[must_use]
    pub fn on_or(&self, id: FieldId, fallback: bool) -> bool {
        match self.get(id) {
            Some(Field::Toggle { on, .. }) => *on,
            _ => fallback,
        }
    }
}
