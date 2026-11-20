use crate::argument_error::ArgumentError;

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error(transparent)]
    Arguments(#[from] ArgumentError),
    #[error(transparent)]
    Host(Box<dyn std::error::Error + Send + Sync>),
}

impl ToolError {
    pub fn host(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Host(Box::new(error))
    }
}
