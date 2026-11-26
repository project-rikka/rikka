#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    #[default]
    Preference,
    State,
}

impl Role {
    #[must_use]
    pub fn is_preference(self) -> bool {
        self == Self::Preference
    }
}
