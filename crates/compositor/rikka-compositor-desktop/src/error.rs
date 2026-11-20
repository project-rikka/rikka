use rikka_compositor_shell::{LayerId, LockSurfaceId, PopupId, ToplevelId};
use rikka_compositor_types::OutputId;

use crate::ids::WindowId;
use crate::window::WindowKind;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("unknown window {0:?}")]
    UnknownWindow(WindowId),
    #[error("unknown output {0:?}")]
    UnknownOutput(OutputId),
    #[error("unknown layer surface {0:?}")]
    UnknownLayer(LayerId),
    #[error("unknown popup {0:?}")]
    UnknownPopup(PopupId),
    #[error("no window for toplevel {0:?}")]
    UnknownToplevel(ToplevelId),
    #[error("{0:?} already has a window")]
    DuplicateWindow(WindowKind),
    #[error("output {0:?} already exists")]
    DuplicateOutput(OutputId),
    #[error("layer surface {0:?} already exists")]
    DuplicateLayer(LayerId),
    #[error("popup {0:?} already exists")]
    DuplicatePopup(PopupId),
    #[error("lock surface {0:?} already exists")]
    DuplicateLockSurface(LockSurfaceId),
    #[error("workspace {number} out of range: each output has {count}")]
    NoSuchWorkspace { number: usize, count: usize },
    #[error("{0:?} is unmanaged and has no workspace")]
    Unmanaged(WindowId),
    #[error("cannot stack {window:?} relative to {other:?}: one is managed, the other unmanaged")]
    SeparateStacks { window: WindowId, other: WindowId },
    #[error("{0:?} is exiting")]
    Exiting(WindowId),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
