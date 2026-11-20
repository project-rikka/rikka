mod action;
mod controller;
mod lifecycle;
mod prompt;
mod tabs;

pub(crate) use controller::attach_actions;

pub use action::{ActionChange, ActionKind, PopupOpening};
pub use controller::{ExtensionIcon, Extensions};
pub use lifecycle::{ExtensionChange, InstallFailure, Lifecycle};
pub use prompt::{InstallRequest, OptionalPermissionsRequest, UpdateRequest};
pub use tabs::{NewTabRequest, OptionsPageRequest, TabCloseRequest, TabUpdateRequest};
