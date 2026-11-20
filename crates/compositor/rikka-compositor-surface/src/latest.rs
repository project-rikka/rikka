use rikka_compositor_geometry::Transform;

use crate::buffer::Buffers;
use crate::color::ColorRepresentation;
use crate::geometry::Geometry;
use crate::ids::BufferId;
use crate::state::Pending;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Latest {
    pub(crate) buffer: Option<BufferId>,
    pub(crate) geometry: Geometry,
    pub(crate) representation: ColorRepresentation,
}

impl Default for Latest {
    fn default() -> Self {
        Self {
            buffer: None,
            geometry: Geometry {
                buffer: None,
                scale: 1,
                transform: Transform::Normal,
                source: None,
                destination: None,
            },
            representation: ColorRepresentation::default(),
        }
    }
}

impl Latest {
    pub(crate) fn after(&self, pending: &Pending, buffers: &Buffers) -> Latest {
        let update = &pending.update;
        let (buffer, size) = match pending.buffer {
            Some(attached) => (attached, attached.and_then(|buffer| buffers.size(buffer))),
            None => (self.buffer, self.geometry.buffer),
        };
        Latest {
            buffer,
            geometry: Geometry {
                buffer: size,
                scale: update.scale.unwrap_or(self.geometry.scale),
                transform: update.transform.unwrap_or(self.geometry.transform),
                source: update.source.unwrap_or(self.geometry.source),
                destination: update.destination.unwrap_or(self.geometry.destination),
            },
            representation: update.color.representation_over(&self.representation),
        }
    }
}
