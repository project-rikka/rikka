pub const DEFAULT_LAYOUT: &str = "us";

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InputSource {
    pub layout: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub variant: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub engine: String,
}

impl InputSource {
    #[must_use]
    pub fn layout(layout: impl Into<String>) -> Self {
        Self {
            layout: layout.into(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn variant(mut self, variant: impl Into<String>) -> Self {
        self.variant = variant.into();
        self
    }

    #[must_use]
    pub fn engine(mut self, id: impl Into<String>) -> Self {
        self.engine = id.into();
        self
    }

    #[must_use]
    pub fn is_plain(&self) -> bool {
        self.engine.is_empty()
    }
}
