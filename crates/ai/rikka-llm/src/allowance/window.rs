#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Span {
    Session,
    Daily,
    Weekly,
    Monthly,
    Annual,
    Unnamed,
}

const DRIFT: f64 = 0.05;

const HOUR: u64 = 60 * 60;
const DAY: u64 = 24 * HOUR;

impl Span {
    pub(crate) fn from_seconds(seconds: u64) -> Span {
        [
            (5 * HOUR, Span::Session),
            (DAY, Span::Daily),
            (7 * DAY, Span::Weekly),
            (30 * DAY, Span::Monthly),
            (365 * DAY, Span::Annual),
        ]
        .into_iter()
        .find(|(named, _)| about(seconds, *named))
        .map_or(Span::Unnamed, |(_, span)| span)
    }
}

fn about(seconds: u64, named: u64) -> bool {
    let (seconds, named) = (seconds as f64, named as f64);
    seconds >= named * (1.0 - DRIFT) && seconds <= named * (1.0 + DRIFT)
}

const WARNING_PERCENT: f64 = 75.0;
const CRITICAL_PERCENT: f64 = 90.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageLevel {
    Normal,
    Warning,
    Critical,
}

impl UsageLevel {
    #[must_use]
    pub fn from_percent(used_percent: f64) -> UsageLevel {
        match used_percent {
            used if used >= CRITICAL_PERCENT => UsageLevel::Critical,
            used if used >= WARNING_PERCENT => UsageLevel::Warning,
            _ => UsageLevel::Normal,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    Everything,
    Part(String),
    Extra,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Window {
    scope: Scope,
    span: Span,
    used_percent: f64,
    resets_at: Option<u64>,
}

impl Window {
    pub(crate) fn new(
        scope: Scope,
        span: Span,
        used_percent: f64,
        resets_at: Option<u64>,
    ) -> Window {
        Window {
            scope,
            span,
            used_percent: used_percent.clamp(0.0, 100.0),
            resets_at,
        }
    }

    #[must_use]
    pub fn scope(&self) -> &Scope {
        &self.scope
    }

    #[must_use]
    pub fn span(&self) -> Span {
        self.span
    }

    #[must_use]
    pub fn used_percent(&self) -> f64 {
        self.used_percent
    }

    #[must_use]
    pub fn resets_at(&self) -> Option<u64> {
        self.resets_at
    }

    #[must_use]
    pub fn usage_level(&self) -> UsageLevel {
        UsageLevel::from_percent(self.used_percent)
    }
}
