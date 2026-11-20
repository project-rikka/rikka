use animate_stage::StageError;

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("cannot start the export thread")]
    Thread(#[source] std::io::Error),
    #[error("cannot create the export renderer")]
    Renderer(#[source] rikka_renderer::Error),
    #[error("cannot render the stage")]
    Stage(#[from] StageError),
    #[error("cannot encode the video")]
    Encode(#[from] rikka_video::Error),
    #[error("the encoder stopped unexpectedly")]
    EncoderPanicked,
    #[error("the encoder stopped before the last frame")]
    EncoderStopped,
}
