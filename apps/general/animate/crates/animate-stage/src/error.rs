use crate::physics::ClothError;

#[derive(Debug, thiserror::Error)]
pub enum StageError {
    #[error("cannot upload a model")]
    Upload(#[source] rikka_model_gpu::Error),
    #[error("cannot draw the stage")]
    Drawing(#[source] rikka_renderer_3d::Error),
    #[error("a rendered frame could not be read back")]
    Readback,
    #[error("cannot compute liquids")]
    Liquid(#[source] rikka_physics::Error),
    #[error("cannot run the cloth simulation")]
    Cloth(#[source] ClothError),
    #[error("cannot apply the look: {0}")]
    Look(String),
}

impl From<rikka_renderer_3d::Error> for StageError {
    fn from(error: rikka_renderer_3d::Error) -> Self {
        Self::Drawing(error)
    }
}

impl From<rikka_renderer::Error> for StageError {
    fn from(error: rikka_renderer::Error) -> Self {
        Self::Drawing(error.into())
    }
}
