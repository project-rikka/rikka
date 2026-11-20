#[derive(Debug, thiserror::Error)]
pub enum ArgumentError {
    #[error("{0}")]
    Refused(String),
    #[error("malformed arguments: {0}")]
    Malformed(#[source] serde_json::Error),
    #[error("edit {edit}: {source}")]
    AtEdit {
        edit: usize,
        #[source]
        source: Box<ArgumentError>,
    },
    #[error("frame {frame}: {source}")]
    AtFrame {
        frame: u32,
        #[source]
        source: Box<ArgumentError>,
    },
    #[error("{what} is not a number")]
    NotFinite { what: String },
    #[error("{what} is {value}{unit}, outside {min}{unit} to {max}{unit}")]
    OutOfRange {
        what: String,
        value: f32,
        min: f32,
        max: f32,
        unit: &'static str,
    },
    #[error("there is no {kind} \"{name}\"; {expected}")]
    Unknown {
        kind: &'static str,
        name: String,
        expected: String,
    },
    #[error("{what} takes {expected}, not {found}")]
    WrongType {
        what: String,
        expected: String,
        found: String,
    },
    #[error("nothing to change; give {expected}")]
    Empty { expected: &'static str },
    #[error("nothing to change on \"{piece}\"; give a preset, enabled, feel or reset")]
    EmptyPiece { piece: String },
    #[error("frames from {from} to {to} hold nothing")]
    EmptyRange { from: u32, to: u32 },
    #[error("`{action}` needs {field}")]
    MissingField {
        action: &'static str,
        field: &'static str,
    },
    #[error("{joint} has no \"{dof}\"; use bend, side or twist")]
    UnknownAxis { joint: String, dof: String },
    #[error("{joint} only turns by {allowed}")]
    FixedAxis { joint: String, allowed: String },
    #[error("{block} is always there; change its settings instead")]
    FixedBlock { block: String },
    #[error("{name} is numbered; name it {block}1{suffix}, {block}2{suffix} and so on")]
    Unnumbered {
        name: String,
        block: String,
        suffix: String,
    },
    #[error("{name} belongs to the whole scene; per figure only {allowed} change")]
    NotPerFigure { name: String, allowed: String },
    #[error("{subject} keys take {expected}, not {found}")]
    WrongKeyEdit {
        subject: &'static str,
        expected: &'static str,
        found: &'static str,
    },
    #[error("{count} frames is more than the {most} one sheet holds; render them in more calls")]
    TooManyFrames { count: usize, most: usize },
    #[error("the eye stands on the target")]
    EyeOnTarget,
    #[error("a distance of {distance} m stands on the target")]
    DistanceOnTarget { distance: f32 },
    #[error("an elevation of {elevation}° looks straight along the vertical; stay within ±89°")]
    VerticalElevation { elevation: f32 },
    #[error("place the camera by `eye`, or by all of `yaw`, `elevation` and `distance`")]
    IncompletePlacement,
}

impl ArgumentError {
    pub(crate) fn at_frame(frame: u32) -> impl Fn(ArgumentError) -> ArgumentError {
        move |error| ArgumentError::AtFrame {
            frame,
            source: Box::new(error),
        }
    }
}
