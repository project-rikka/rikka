mod completion;
mod constraints;
mod cursor;
mod host;
mod module;
mod protocol;
mod store;
mod types;

pub use host::CaptureHost;
pub use module::CaptureProtocols;
pub use rikka_compositor_protocols::ext_image_copy_capture_frame_v1::FailureReason as CaptureFailure;
pub use store::Captures;
pub use types::{
    CaptureFormats, CaptureRequest, CaptureSource, Constraints, CursorPlacement, DmabufConstraints,
    DmabufFormat, FrameReady, SessionInfo,
};
