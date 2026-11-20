#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ServerConfigError {
    MissingName,
    MissingCommand,
    MissingUrl,
    NonHttpUrl,
}

impl ServerConfigError {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            ServerConfigError::MissingName => "missing name",
            ServerConfigError::MissingCommand => "missing command",
            ServerConfigError::MissingUrl => "missing URL",
            ServerConfigError::NonHttpUrl => "URL scheme is not http or https",
        }
    }
}

impl std::fmt::Display for ServerConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}
