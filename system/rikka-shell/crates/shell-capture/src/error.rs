#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no output to capture")]
    NoScreen,

    #[error("nothing was selected")]
    NothingSelected,

    #[error("no clipboard")]
    NoClipboard,

    #[error("no XDG pictures or videos directory")]
    Nowhere,

    #[error("no output directory: {0}")]
    NoFolder(#[source] std::io::Error),

    #[error("the picker is up; finish or cancel it first")]
    Busy,

    #[error("a recording is already running")]
    AlreadyRecording,

    #[error(transparent)]
    Capture(#[from] rikka_capture::Error),

    #[error(transparent)]
    Record(#[from] rikka_record::Error),

    #[error(transparent)]
    Picture(#[from] rikka_image::Error),

    #[error(transparent)]
    Clipboard(#[from] rikka_clipboard::Error),

    #[error(transparent)]
    Barcode(#[from] rikka_barcode::Error),
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;
