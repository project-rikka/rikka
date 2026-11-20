use rikka_gpu::reclaim::Reclaimer;

use crate::material::MaterialId;
use crate::mesh::MeshId;
use crate::overlay::OverlayBufferId;
use crate::viewport::ViewportParts;

pub(crate) enum Reclaimed {
    Mesh(MeshId),
    Material(MaterialId),
    OverlayBuffer(OverlayBufferId),
    Viewport(Box<ViewportParts>),
}

pub trait Reclaim3d {
    fn mesh(&self, id: MeshId);

    fn material(&self, id: MaterialId);

    fn overlay_buffer(&self, id: OverlayBufferId);
}

impl Reclaim3d for Reclaimer {
    fn mesh(&self, id: MeshId) {
        self.route(Reclaimed::Mesh(id));
    }

    fn material(&self, id: MaterialId) {
        self.route(Reclaimed::Material(id));
    }

    fn overlay_buffer(&self, id: OverlayBufferId) {
        self.route(Reclaimed::OverlayBuffer(id));
    }
}
