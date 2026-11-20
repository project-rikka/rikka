use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure(String);

impl Failure {
    pub fn new(message: impl Into<String>) -> Failure {
        Failure(message.into())
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl<E: std::error::Error> From<E> for Failure {
    fn from(error: E) -> Failure {
        Failure(error.to_string())
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ErrorBanner(Option<Failure>);

impl ErrorBanner {
    pub fn set(&mut self, failure: impl Into<Failure>) {
        self.0 = Some(failure.into());
    }

    pub fn clear(&mut self) {
        self.0 = None;
    }

    #[must_use]
    pub fn message(&self) -> Option<&str> {
        self.0.as_ref().map(Failure::message)
    }

    pub fn capture<T>(&mut self, outcome: Result<T, Failure>) -> Option<T> {
        match outcome {
            Ok(value) => Some(value),
            Err(failure) => {
                self.0 = Some(failure);
                None
            }
        }
    }
}
