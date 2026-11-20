use animate_stage::StageError;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ServeError {
    #[error("there are no figures in the scene")]
    NoFigures,
    #[error("there is no figure \"{name}\"; the figures are {figures}")]
    UnknownFigure { name: String, figures: String },
    #[error("\"{name}\" was removed from the scene")]
    FigureRemoved { name: String },
    #[error("\"{name}\" has no humanoid skeleton, so it cannot be posed by parts")]
    NotHumanoid { name: String },
    #[error("frame {frame}: \"{figure}\" has no morph \"{morph}\"; its morphs are {morphs}")]
    UnknownMorph {
        frame: u32,
        figure: String,
        morph: String,
        morphs: String,
    },
    #[error(
        "frame {frame}: \"{figure}\" has no morphs for the expression \"{expression}\"; use `morphs`"
    )]
    MissingExpression {
        frame: u32,
        figure: String,
        expression: &'static str,
    },
    #[error(transparent)]
    Value(#[from] animate_assistant::ArgumentError),
    #[error("\"{figure}\" has no cloth or hair pieces")]
    NoPieces { figure: String },
    #[error("\"{figure}\" has no piece \"{piece}\"; its pieces are {pieces}")]
    UnknownPiece {
        figure: String,
        piece: String,
        pieces: String,
    },
    #[error("cannot analyze the cloth")]
    Cloth(#[source] std::sync::Arc<rikka_physics_tailor::Error>),
    #[error("cannot render the frames")]
    Capture(#[source] StageError),
    #[error("the stage closed before it drew")]
    StageClosed,
    #[error("the stage did not draw in time; the window may be hidden or minimised")]
    RenderTimeout,
    #[error("cannot assemble the contact sheet")]
    ContactSheet(#[source] rikka_image::Error),
    #[error("cannot assemble the contact sheet")]
    ContactSheetTask(#[source] tokio::task::JoinError),
    #[error("the window closed")]
    WindowClosed,
    #[error("the window closed before replying")]
    NoReply,
}
