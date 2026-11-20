use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldKey {
    Backspace,
    Tab,
    Caps,
    Shift,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
}

impl FieldKey {
    fn name(self) -> &'static str {
        match self {
            FieldKey::Backspace => "Backspace",
            FieldKey::Tab => "Tab",
            FieldKey::Caps => "Caps",
            FieldKey::Shift => "Shift",
            FieldKey::ArrowLeft => "ArrowLeft",
            FieldKey::ArrowRight => "ArrowRight",
            FieldKey::ArrowUp => "ArrowUp",
            FieldKey::ArrowDown => "ArrowDOWN",
        }
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct FieldActivity {
    pub(crate) focus_seconds: Vec<u64>,
    pub(crate) clicks: u32,
    pub(crate) copies: Vec<String>,
    pub(crate) pastes: Vec<String>,
    pub(crate) keys: Vec<String>,
}

impl FieldActivity {
    #[must_use]
    pub fn new() -> FieldActivity {
        FieldActivity::default()
    }

    pub fn record_focus(&mut self, duration: Duration) {
        self.focus_seconds.push(duration.as_secs());
    }

    pub fn record_click(&mut self) {
        self.clicks = self.clicks.saturating_add(1);
    }

    pub fn record_input(&mut self, text: &str) {
        self.keys.extend(text.chars().map(String::from));
    }

    pub fn record_key_press(&mut self, key: FieldKey) {
        self.keys.push(key.name().to_owned());
    }

    pub fn record_copy(&mut self, text: impl Into<String>) {
        self.keys.push("Copy".to_owned());
        self.copies.push(text.into());
    }

    pub fn record_paste(&mut self, text: impl Into<String>) {
        self.keys.push("Paste".to_owned());
        self.pastes.push(text.into());
    }
}

impl std::fmt::Debug for FieldActivity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FieldActivity")
            .field("focus_seconds", &self.focus_seconds)
            .field("clicks", &self.clicks)
            .field("keys", &self.keys.len())
            .finish_non_exhaustive()
    }
}
