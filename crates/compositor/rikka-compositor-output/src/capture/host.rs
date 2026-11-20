use std::fmt;

use rikka_compositor_protocols::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1;
use rikka_compositor_surface::SurfaceHost;
use rikka_wayland_wire::Key;

use super::store::Captures;
use super::types::{CaptureRequest, Constraints, SessionInfo};
use crate::ids::CaptureSessionId;
use crate::outputs::OutputHost;

pub trait CaptureHost: SurfaceHost + OutputHost {
    type Toplevel: Key + Eq + fmt::Debug;

    fn captures(&mut self) -> &mut Captures<Self::Toplevel>;

    fn toplevel_source(&mut self, handle: &ExtForeignToplevelHandleV1) -> Option<Self::Toplevel>;

    fn capture_session_started(
        &mut self,
        session: CaptureSessionId,
        info: SessionInfo<Self::Toplevel>,
    ) -> Option<Constraints>;

    fn capture_requested(&mut self, request: CaptureRequest<Self::Toplevel>);
}
