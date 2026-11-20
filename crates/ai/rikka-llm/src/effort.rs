#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    #[serde(rename = "none")]
    Off,
    Minimal,
    Low,
    #[default]
    Medium,
    High,
    XHigh,
    Max,
}

impl Effort {
    pub const ALL: [Effort; 7] = [
        Effort::Off,
        Effort::Minimal,
        Effort::Low,
        Effort::Medium,
        Effort::High,
        Effort::XHigh,
        Effort::Max,
    ];

    #[must_use]
    pub fn id(self) -> &'static str {
        self.name().unwrap_or("none")
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Effort> {
        Effort::ALL.into_iter().find(|effort| effort.id() == id)
    }

    #[must_use]
    pub fn name(self) -> Option<&'static str> {
        match self {
            Effort::Off => None,
            Effort::Minimal => Some("minimal"),
            Effort::Low => Some("low"),
            Effort::Medium => Some("medium"),
            Effort::High => Some("high"),
            Effort::XHigh => Some("xhigh"),
            Effort::Max => Some("max"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Reveal {
    #[default]
    Hidden,
    Summary,
}
