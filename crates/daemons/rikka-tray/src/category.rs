#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum Category {
    Hardware,
    SystemServices,
    Communications,
    #[default]
    ApplicationStatus,
}

impl Category {
    #[must_use]
    pub(crate) fn named(name: &str) -> Category {
        match name {
            "Hardware" => Category::Hardware,
            "SystemServices" => Category::SystemServices,
            "Communications" => Category::Communications,
            _ => Category::ApplicationStatus,
        }
    }
}
