use glam::{Vec2, Vec4};

use crate::ids::{EdgeId, LoopId};
use crate::math::{color_equal, uv_equal};
use crate::mesh::EditMesh;
use crate::{DataType, Domain};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MathLayerKind {
    Vec2,
    Color,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MathLayer {
    pub name: String,
    pub kind: MathLayerKind,
}

impl MathLayer {
    #[must_use]
    pub fn loops_equal(&self, mesh: &EditMesh, a: LoopId, b: LoopId) -> bool {
        match self.kind {
            MathLayerKind::Vec2 => mesh
                .attribute::<Vec2>(&self.name, Domain::Corner)
                .is_none_or(|values| uv_equal(values[a.index()], values[b.index()])),
            MathLayerKind::Color => mesh
                .attribute::<Vec4>(&self.name, Domain::Corner)
                .is_none_or(|values| color_equal(values[a.index()], values[b.index()])),
        }
    }

    pub fn copy(&self, mesh: &mut EditMesh, from: LoopId, to: LoopId) {
        match self.kind {
            MathLayerKind::Vec2 => {
                if let Some(values) = mesh.attribute_mut::<Vec2>(&self.name, Domain::Corner) {
                    values[to.index()] = values[from.index()];
                }
            }
            MathLayerKind::Color => {
                if let Some(values) = mesh.attribute_mut::<Vec4>(&self.name, Domain::Corner) {
                    values[to.index()] = values[from.index()];
                }
            }
        }
    }
}

impl EditMesh {
    #[must_use]
    pub fn loop_math_layers(&self) -> Vec<MathLayer> {
        self.attributes(Domain::Corner)
            .filter_map(|attribute| {
                let kind = match attribute.data_type() {
                    DataType::Vec2 => MathLayerKind::Vec2,
                    DataType::Color => MathLayerKind::Color,
                    _ => return None,
                };
                Some(MathLayer {
                    name: attribute.name.clone(),
                    kind,
                })
            })
            .collect()
    }

    #[must_use]
    pub fn has_loop_math(&self) -> bool {
        self.attributes(Domain::Corner)
            .any(|attribute| matches!(attribute.data_type(), DataType::Vec2 | DataType::Color))
    }

    #[must_use]
    pub fn is_edge_contiguous_loop_data(&self, edge: EdgeId, layer: &MathLayer) -> bool {
        let Some(base) = self.edge_first_loop(edge) else {
            return true;
        };
        if self.loop_radial_next(base) == base {
            return true;
        }
        let base_next = self.loop_next(base);
        let base_vert = self.loop_vert(base);
        let mut other = self.loop_radial_next(base);
        while other != base {
            let (at_v1, at_v2) = match self.loop_vert(other) == base_vert {
                true => (other, self.loop_next(other)),
                false => (self.loop_next(other), other),
            };
            if !layer.loops_equal(self, base, at_v1) || !layer.loops_equal(self, base_next, at_v2) {
                return false;
            }
            other = self.loop_radial_next(other);
        }
        true
    }
}
