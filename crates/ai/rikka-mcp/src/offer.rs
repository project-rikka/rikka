#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    name: String,
    description: Option<String>,
}

impl Offer {
    pub(crate) fn new(name: String, description: Option<String>) -> Offer {
        Offer { name, description }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}
