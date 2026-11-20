use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Timeout {
    #[default]
    Default,
    Never,
    After(Duration),
}

impl Timeout {
    #[must_use]
    pub(crate) fn from_millis(milliseconds: i32) -> Timeout {
        match milliseconds {
            0 => Timeout::Never,
            span if span < 0 => Timeout::Default,
            span => Timeout::After(Duration::from_millis(span as u64)),
        }
    }

    #[must_use]
    pub fn against(self, default: Duration) -> Option<Duration> {
        match self {
            Timeout::Default => Some(default),
            Timeout::Never => None,
            Timeout::After(span) => Some(span),
        }
    }
}
