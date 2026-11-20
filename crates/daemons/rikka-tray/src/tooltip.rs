use crate::icon::Icon;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tooltip {
    pub icon: Option<Icon>,
    pub title: String,
    pub description: String,
}

impl Tooltip {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.title.is_empty() && self.description.is_empty()
    }
}
