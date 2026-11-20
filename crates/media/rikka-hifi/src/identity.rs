#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub(crate) name: String,
    pub(crate) id: String,
    pub(crate) icon: Option<String>,
}

impl Identity {
    #[must_use]
    pub fn new(name: impl Into<String>, id: impl Into<String>) -> Identity {
        Identity {
            name: name.into(),
            id: id.into(),
            icon: None,
        }
    }

    #[must_use]
    pub fn icon(mut self, icon: impl Into<String>) -> Identity {
        self.icon = Some(icon.into());
        self
    }
}
