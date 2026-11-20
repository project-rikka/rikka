#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyboardState {
    pub caps_lock: bool,
    pub layout: Option<String>,
    pub layouts: u32,
}

impl KeyboardState {
    #[must_use]
    pub fn has_multiple_layouts(&self) -> bool {
        self.layouts > 1
    }
}
