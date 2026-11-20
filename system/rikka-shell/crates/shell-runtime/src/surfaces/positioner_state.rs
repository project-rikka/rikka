use rikka_types::{Point, Size};
use rikka_wayland_client::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;
use rikka_wayland_client::{ObjectId, Proxy};

use super::Surfaces;
use crate::convert;
use crate::screens::Screens;
use shell_kit::{Anchor, ExclusiveZone, KeyboardMode};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PositionerState {
    Hidden,
    Asked(PopupGeometry),
    Shown(PopupGeometry),
    Hiding,
    Unmapping,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PopupGeometry {
    at: Point,
    size: (u32, u32),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Unmapping(pub(crate) ObjectId);

impl rikka_wayland_client::Key for Unmapping {
    fn into_raw(self) -> u64 {
        self.0.into_raw()
    }

    fn from_raw(raw: u64) -> Unmapping {
        Unmapping(ObjectId::from_raw(raw))
    }
}

impl PositionerState {
    pub(crate) fn offered(self) -> bool {
        matches!(self, Self::Asked(_) | Self::Shown(_) | Self::Hiding)
    }
}

impl PopupGeometry {
    fn new(at: Point, size: Size) -> PopupGeometry {
        PopupGeometry {
            at,
            size: (
                size.width.round().max(1.0) as u32,
                size.height.round().max(1.0) as u32,
            ),
        }
    }

    fn place(self, role: &ZwlrLayerSurfaceV1) {
        role.set_size(self.size.0, self.size.1);
        role.set_margin(self.at.y as i32, 0, 0, self.at.x as i32);
    }
}

impl Surfaces {
    pub(super) fn follow_positioners(&mut self, screens: &Screens) {
        let positioned: Vec<ObjectId> = self
            .embedder
            .iter()
            .filter(|(_, hosted)| hosted.state.positioner_state.is_some())
            .map(|(id, _)| *id)
            .collect();
        for id in positioned {
            self.positioner(&id, screens);
        }
    }

    fn positioner(&mut self, id: &ObjectId, screens: &Screens) {
        let Some(hosted) = self.embedder.get(id) else {
            return;
        };
        let settings = &self.descs[hosted.state.desc].settings;
        let (Some(positioner), Some(positioner_state)) =
            (settings.positioner.as_ref(), hosted.state.positioner_state)
        else {
            return;
        };
        let size = hosted.state.wanted_size(Size::new(1.0, 1.0));
        let geometry = hosted
            .state
            .output
            .as_ref()
            .and_then(|output| screens.placement_of(output))
            .and_then(|placement| positioner.geometry_for(&placement, size))
            .map(|at| PopupGeometry::new(at, size));
        let keyboard = settings.keyboard;
        match (positioner_state, geometry) {
            (PositionerState::Hidden, Some(geometry)) => self.offer(id, geometry, keyboard),
            (PositionerState::Asked(current) | PositionerState::Shown(current), Some(geometry))
                if current == geometry => {}
            (PositionerState::Asked(_), Some(geometry)) => self.offer(id, geometry, keyboard),
            (PositionerState::Shown(_) | PositionerState::Hiding, Some(geometry)) => {
                self.move_to(id, geometry)
            }
            (PositionerState::Shown(_) | PositionerState::Hiding, None) => self.hide(id),
            (
                PositionerState::Hidden | PositionerState::Asked(_) | PositionerState::Unmapping,
                None,
            )
            | (PositionerState::Unmapping, Some(_)) => {}
        }
    }

    fn offer(&mut self, id: &ObjectId, geometry: PopupGeometry, keyboard: KeyboardMode) {
        let Some(hosted) = self.embedder.get_mut(id) else {
            return;
        };
        let Some(role) = hosted.state.role.layer() else {
            return;
        };
        geometry.place(role);
        role.set_anchor(convert::anchor(Anchor::TOP | Anchor::LEFT));
        role.set_exclusive_zone(convert::exclusive_zone(ExclusiveZone::Ignore));
        role.set_keyboard_interactivity(convert::keyboard_mode(keyboard));
        hosted.state.role.commit();
        hosted.state.positioner_state = Some(PositionerState::Asked(geometry));
    }

    fn move_to(&mut self, id: &ObjectId, geometry: PopupGeometry) {
        let Some(hosted) = self.embedder.get_mut(id) else {
            return;
        };
        if let Some(role) = hosted.state.role.layer() {
            geometry.place(role);
        }
        hosted.state.role.commit();
        hosted.drawable(true);
        hosted.state.positioner_state = Some(PositionerState::Shown(geometry));
    }

    fn hide(&mut self, id: &ObjectId) {
        let Some(hosted) = self.embedder.get_mut(id) else {
            return;
        };
        hosted.drawable(false);
        if hosted.is_painting() {
            hosted.state.positioner_state = Some(PositionerState::Hiding);
            return;
        }
        let surface = hosted.state.role.surface();
        surface.attach(None, 0, 0);
        surface.commit();
        self.host
            .display()
            .sync(&self.route)
            .set_key(Unmapping(surface.object_id()));
        hosted.frame_answered();
        hosted.state.configured = false;
        hosted.state.positioner_state = Some(PositionerState::Unmapping);
    }

    pub(crate) fn unmapped(&mut self, id: &ObjectId) {
        if let Some(hosted) = self.embedder.get_mut(id)
            && hosted.state.positioner_state == Some(PositionerState::Unmapping)
        {
            hosted.state.positioner_state = Some(PositionerState::Hidden);
        }
    }
}
