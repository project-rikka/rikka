use serde::Deserialize;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct SievePosition {
    #[serde(alias = "Line", default)]
    line: u32,
    #[serde(alias = "ch", alias = "Ch", alias = "Column", default)]
    column: u32,
}

impl SievePosition {
    #[must_use]
    pub fn line(&self) -> u32 {
        self.line
    }

    #[must_use]
    pub fn column(&self) -> u32 {
        self.column
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct SieveIssue {
    #[serde(alias = "Message", default)]
    message: String,
    #[serde(alias = "Severity", default)]
    severity: String,
    #[serde(alias = "From", default)]
    from: Option<SievePosition>,
    #[serde(alias = "To", default)]
    to: Option<SievePosition>,
}

impl SieveIssue {
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn severity(&self) -> &str {
        &self.severity
    }

    #[must_use]
    pub fn is_error(&self) -> bool {
        self.severity.is_empty() || self.severity.eq_ignore_ascii_case("error")
    }

    #[must_use]
    pub fn start(&self) -> Option<SievePosition> {
        self.from
    }

    #[must_use]
    pub fn end(&self) -> Option<SievePosition> {
        self.to
    }
}
