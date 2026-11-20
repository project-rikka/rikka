use rikka_blur::{Glass, SurfaceRegions};
use rikka_types::Size;
use rikka_wayland_client::ObjectId;
use rikka_wayland_client::wl_output::WlOutput;
use rikka_window_view::WindowViews;

use super::embedded_views::EmbeddedViews;
use super::opacity::{AlphaMultiplier, Driving};
use super::role::Role;
use super::surfaces::{Area, PositionerState, SurfaceId};
use shell_kit::hosting::Measured;

pub(super) struct SurfaceState {
    pub(crate) desc: SurfaceId,
    pub(crate) output: Option<ObjectId>,
    pub(super) entered: Vec<WlOutput>,
    pub(crate) logical: Size,
    pub(crate) scale: i32,
    pub(crate) configured: bool,
    pub(crate) fit: Option<Measured>,
    pub(crate) room: Size,
    pub(super) glazing: SurfaceRegions,
    pub(crate) alpha: Option<AlphaMultiplier>,
    pub(crate) driving: Option<Driving>,
    pub(crate) panes: Vec<Area>,
    pub(super) embedded_views: Option<EmbeddedViews>,
    pub(super) positioner_state: Option<PositionerState>,
    pub(crate) role: Role,
}

pub(super) struct SurfaceEffects<'a> {
    pub(crate) glass: Option<&'a Glass>,
    pub(crate) blurs: bool,
    pub(super) window_views: Option<&'a WindowViews>,
}

impl SurfaceState {
    pub(crate) fn new(
        desc: SurfaceId,
        output: Option<ObjectId>,
        role: Role,
        fit: Option<Measured>,
        room: Size,
        behind: SurfaceEffects<'_>,
    ) -> Self {
        let blurred = behind.glass.map(|glass| glass.for_surface(role.surface()));
        let embedded_views = behind
            .window_views
            .map(|views| EmbeddedViews::new(views.inside(role.surface())));
        Self {
            desc,
            output,
            entered: Vec::new(),
            logical: Size::new(0.0, 0.0),
            scale: 1,
            configured: false,
            fit,
            room,
            glazing: SurfaceRegions::new(blurred, behind.blurs),
            alpha: None,
            driving: None,
            panes: Vec::new(),
            embedded_views,
            positioner_state: None,
            role,
        }
    }

    #[must_use]
    pub(super) fn wanted_size(&self, declared: Size) -> Size {
        self.fit.as_ref().map_or(declared, Measured::get)
    }

    pub(super) fn set_blur(&mut self, glass: Option<&Glass>, blurs: bool) {
        let surface = self.role.surface();
        self.glazing
            .set_blur(blurs, || glass.map(|glass| glass.for_surface(surface)));
    }
}

impl std::fmt::Debug for SurfaceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SurfaceState")
            .field("desc", &self.desc)
            .field("logical", &self.logical)
            .field("scale", &self.scale)
            .field("configured", &self.configured)
            .finish_non_exhaustive()
    }
}
