use std::path::PathBuf;

use crate::relink::Mismatch;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid frame rate {numerator}/{denominator}")]
    Rate { numerator: u32, denominator: u32 },

    #[error("{} is shorter than one frame", .0.display())]
    TooShort(PathBuf),

    #[error("duration must be at least one frame")]
    Empty,

    #[error("clip is not a shape or text")]
    NotGraphic,

    #[error("clip is not footage")]
    NotFootage,

    #[error("clip is not an effect region")]
    NotRegion,

    #[error("audio-only clip has no picture")]
    NotVisual,

    #[error("clip is not a camera")]
    NotCamera,

    #[error("property is not animatable on this clip")]
    NotAnimatable,

    #[error("property has no spatial tangents")]
    NotSpatial,

    #[error("value does not match the property type")]
    WrongValue,

    #[error("no such effect on clip")]
    UnknownEffect,

    #[error("effect has no parameter {0:?}")]
    UnknownKnob(String),

    #[error("tween needs at least two frames")]
    TooBrief,

    #[error("clip has no tween")]
    NotTweened,

    #[error("no edit point selected")]
    NoEditPoint,

    #[error("no keyframe at that frame")]
    NoKey,

    #[error("clip not found")]
    UnknownClip,

    #[error("media not found")]
    UnknownMedia,

    #[error("{} cannot stand in for the media: {reason}", path.display())]
    Mismatch {
        path: PathBuf,
        #[source]
        reason: Mismatch,
    },

    #[error("no copied attribute fits the selected clips")]
    NothingToPaste,

    #[error("the selected clips have no effects")]
    NoEffects,

    #[error("layer is locked")]
    Locked,

    #[error("edit exceeds the available range")]
    OutOfReach,

    #[error("no clip at that frame")]
    NothingToSplit,

    #[error("marker not found")]
    UnknownMarker,

    #[error("no edit point there")]
    NothingToTrim,

    #[error("invalid frame size {width}×{height}")]
    FrameSize { width: u32, height: u32 },

    #[error(transparent)]
    File(#[from] rikka_file_format::Error),

    #[error("{} is not a project: {reason}", path.display())]
    Damaged { path: PathBuf, reason: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
