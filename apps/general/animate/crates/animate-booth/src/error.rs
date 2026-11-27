use std::sync::Arc;

use animate_stage::StageError;

#[derive(Debug, thiserror::Error)]
pub enum BoothError {
    #[error("cannot start the booth")]
    Spawn(#[source] std::io::Error),
    #[error("cannot read the file")]
    Read(#[source] std::io::Error),
    #[error("cannot import the file")]
    Import(#[from] rikka_model::Error),
    #[error("cannot read the Rikka motion")]
    Motion(#[from] animate_document::Error),
    #[error("the file holds no motion for the performer")]
    NoMotion,
    #[error("the motion needs a humanoid performer")]
    NeedsBody,
    #[error("the file holds no camera motion")]
    NoCamera,
    #[error("the booth renderer is unavailable: {0}")]
    Unavailable(Arc<str>),
    #[error("cannot load the model: {0}")]
    Unseated(Arc<str>),
    #[error("cannot render the picture")]
    Render(#[from] StageError),
}

pub(crate) fn reason(error: &dyn std::error::Error) -> Arc<str> {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text.into()
}
