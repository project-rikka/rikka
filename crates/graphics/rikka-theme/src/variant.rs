#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Variant {
    Light,
    Dark,
}

impl Variant {
    pub fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }
}
