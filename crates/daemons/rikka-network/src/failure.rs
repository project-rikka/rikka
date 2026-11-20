#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Failure {
    #[default]
    None,
    Secrets,
    Missing,
    Handshake,
    Address,
    Carrier,
    Other,
}

impl Failure {
    pub(crate) fn from_code(code: u32) -> Self {
        match code {
            0..=3 | 36..=39 | 41 | 60..=62 => Self::None,
            7 => Self::Secrets,
            53 | 67 => Self::Missing,
            8..=11 | 44 => Self::Handshake,
            5 | 6 | 15..=17 | 20..=22 | 64 => Self::Address,
            40 => Self::Carrier,
            _ => Self::Other,
        }
    }

    #[must_use]
    pub fn happened(self) -> bool {
        self != Self::None
    }
}
