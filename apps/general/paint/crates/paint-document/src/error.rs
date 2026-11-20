use std::io;
use std::path::PathBuf;

use crate::figure::FigureId;
use crate::layer::LayerId;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot read {}", path.display())]
    Unreadable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("unsupported image format: {}", path.display())]
    Unsupported { path: PathBuf },

    #[error("corrupt image {}: {reason}", path.display())]
    Damaged { path: PathBuf, reason: String },

    #[error("{width}×{height} image exceeds the memory limit")]
    TooLarge { width: u32, height: u32 },

    #[error("GPU cannot run image work")]
    NoGpu,

    #[error("image processing failed: {0}")]
    Imaging(#[from] rikka_image_edit::Error),

    #[error("GPU work failed: {0}")]
    Gpu(#[from] rikka_gpu::Error),

    #[error("layered document failed: {0}")]
    Psd(#[from] rikka_psd::Error),

    #[error("picture conversion failed: {0}")]
    Picture(#[from] rikka_image::Error),

    #[error("cannot write {}", path.display())]
    Unwritable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("font family not found: {family}")]
    UnknownFont { family: String },

    #[error("failed to lay out text: {0}")]
    Unlettered(String),

    #[error("background layer is locked")]
    BackgroundLocked,

    #[error("cannot delete the last layer")]
    LastLayer,

    #[error("layer {} not found", .0.0)]
    UnknownLayer(LayerId),

    #[error("layer {} holds no pixels", .0.0)]
    NotPixelLayer(LayerId),

    #[error("layer {} is hidden", .0.0)]
    HiddenLayer(LayerId),

    #[error("no layer is visible")]
    NothingVisible,

    #[error("fewer than two layers are visible")]
    TooFewVisible,

    #[error("image is already flat")]
    AlreadyFlat,

    #[error("image has no Background layer")]
    NoBackground,

    #[error("blend mode cannot be drawn")]
    UnknownBlend,

    #[error("only groups pass their blending through")]
    PassThroughOutsideGroup,

    #[error("layer cannot move inside itself")]
    MoveIntoItself,

    #[error("only groups hold layers")]
    NotGroup,

    #[error("layer is already at the top")]
    AtTop,

    #[error("layer is already at the bottom")]
    AtBottom,

    #[error("group cannot be merged down")]
    GroupMerge,

    #[error("no pixel layer below to merge into")]
    NothingBelow,

    #[error("only RGB documents change their colour profile")]
    ProfileNeedsRgb,

    #[error("32-bit documents cannot change their colour profile")]
    ProfileOnFloat,

    #[error("RGB documents cannot take a grey colour profile")]
    GreyProfile,

    #[error("nothing to add")]
    NothingToAdd,

    #[error("object {} already exists", .0.0)]
    FigureExists(FigureId),

    #[error("object {} not found on that layer", .0.0)]
    UnknownFigure(FigureId),

    #[error("text object has no text")]
    EmptyText,

    #[error("object has no finite position and size")]
    UnboundedFigure,

    #[error("one layer cannot change the size of the whole document")]
    LayerReshape,

    #[error("changes to the whole canvas are not previewed")]
    CanvasPreview,

    #[error("undo, redo and history jumps cannot be part of a batch")]
    TravelInBatch,

    #[error("batch cannot hold another batch")]
    NestedBatch,

    #[error("({x}, {y}) lies outside the {width}×{height} image")]
    OutsideImage {
        x: f32,
        y: f32,
        width: u32,
        height: u32,
    },

    #[error("image needs at least one pixel")]
    EmptyImage,

    #[error("resolution must be positive")]
    BadResolution,

    #[error("resampling cannot be inverted")]
    SingularMap,
}

impl From<rikka_figure::Error> for Error {
    fn from(error: rikka_figure::Error) -> Error {
        match error {
            rikka_figure::Error::UnknownFont { family } => Error::UnknownFont { family },
            rikka_figure::Error::Unlettered(reason) => Error::Unlettered(reason),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
