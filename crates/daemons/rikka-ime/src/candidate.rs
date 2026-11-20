#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Candidate {
    pub label: String,
    pub text: String,
}

impl Candidate {
    #[must_use]
    pub fn new(label: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            text: text.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CandidateLayout {
    #[default]
    Vertical,
    Horizontal,
}

impl CandidateLayout {
    #[must_use]
    pub(crate) fn from_hint(hint: i32) -> Self {
        match hint {
            2 => Self::Horizontal,
            _ => Self::Vertical,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Candidates {
    pub entries: Vec<Candidate>,
    pub focused: Option<usize>,
    pub previous: bool,
    pub more: bool,
    pub layout: CandidateLayout,
}

impl Candidates {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
