use std::error::Error as _;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to build a model material: {0}")]
    Material(String),

    #[error("failed to upload a model mesh: {0}")]
    Mesh(String),

    #[error("the model has no material {0}")]
    NoMaterial(usize),
}

impl Error {
    pub(crate) fn material(error: impl Into<rikka_renderer_3d::Error>) -> Self {
        Self::Material(chain(&error.into()))
    }

    pub(crate) fn mesh(error: rikka_renderer_3d::Error) -> Self {
        Self::Mesh(chain(&error))
    }
}

fn chain(error: &rikka_renderer_3d::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
