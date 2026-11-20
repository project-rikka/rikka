#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PmxVersion {
    #[default]
    V2_0,
    V2_1,
}

impl PmxVersion {
    pub const ALL: [PmxVersion; 2] = [PmxVersion::V2_0, PmxVersion::V2_1];

    #[must_use]
    pub fn number(self) -> f32 {
        match self {
            Self::V2_0 => 2.0,
            Self::V2_1 => 2.1,
        }
    }

    pub(crate) fn from_number(number: f32) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|version| (number - version.number()).abs() < 1e-6)
    }
}
