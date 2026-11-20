#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Engine {
    pub id: String,
    pub name: String,
    pub native_name: String,
    pub icon: String,
    pub label: String,
    pub language: String,
    pub configurable: bool,
}

impl Engine {
    #[must_use]
    pub fn is_plain(&self) -> bool {
        self.id.starts_with(KEYBOARD)
    }
}

const KEYBOARD: &str = "keyboard-";
