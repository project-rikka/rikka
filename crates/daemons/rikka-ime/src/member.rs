#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Member {
    pub engine: String,
    pub layout: String,
}

impl Member {
    #[must_use]
    pub fn new(engine: impl Into<String>) -> Member {
        Member {
            engine: engine.into(),
            layout: String::new(),
        }
    }

    #[must_use]
    pub fn with_layout(mut self, layout: impl Into<String>) -> Member {
        self.layout = layout.into();
        self
    }
}
