use rikka_model_core::glam::{Mat4, Vec3};
use rikka_model_core::{Instance, Mesh, Model, NodeId};
use rikka_physics_tailor::VertexKey;

use crate::ray::Ray;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub point: Vec3,
    pub distance: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Surface {
    keys: Vec<VertexKey>,
    points: Vec<Vec3>,
    normals: Vec<Vec3>,
    triangles: Vec<[u32; 3]>,
}

impl Surface {
    #[must_use]
    pub fn from_instance(instance: &Instance) -> Surface {
        let model = instance.model();
        let transform = instance.transform();
        Surface::skinned(model, |node| {
            let world = transform * instance.world(node);
            let palette = model.nodes[node.index()]
                .skin
                .map(|skin| {
                    instance
                        .joints(skin)
                        .iter()
                        .map(|joint| transform * *joint)
                        .collect()
                })
                .unwrap_or_default();
            Placing {
                world,
                palette,
                morphs: instance.morph_weights(node).to_vec(),
            }
        })
    }

    #[must_use]
    pub fn rest(model: &Model) -> Surface {
        let worlds = model.world_transforms();
        Surface::skinned(model, |node| {
            let palette = model.nodes[node.index()]
                .skin
                .map(|skin| {
                    let skin = model.skin(skin);
                    skin.joints
                        .iter()
                        .zip(&skin.inverse_bind)
                        .map(|(joint, bind)| worlds[joint.index()] * *bind)
                        .collect()
                })
                .unwrap_or_default();
            Placing {
                world: worlds[node.index()],
                palette,
                morphs: Vec::new(),
            }
        })
    }

    fn skinned(model: &Model, placing: impl Fn(NodeId) -> Placing) -> Surface {
        let mut surface = Surface::default();
        for (index, node) in model.nodes.iter().enumerate() {
            let Some(mesh) = node.mesh else {
                continue;
            };
            let node_id = NodeId(index as u32);
            let mesh = model.mesh(mesh);
            let skinned = node.skin.is_some() && mesh.vertices.is_skinned();
            surface.place(node_id, mesh, &placing(node_id), skinned);
        }
        surface
    }

    fn place(&mut self, node: NodeId, mesh: &Mesh, placing: &Placing, skinned: bool) {
        let first = self.points.len() as u32;
        let vertices = &mesh.vertices;
        let mut positions = vertices.positions.clone();
        for (target, weight) in mesh.morph_targets.iter().zip(&placing.morphs) {
            if *weight == 0.0 {
                continue;
            }
            for (vertex, delta) in target.vertices.iter().zip(&target.positions) {
                if let Some(position) = positions.get_mut(*vertex as usize) {
                    *position += *delta * *weight;
                }
            }
        }
        for (index, position) in positions.iter().enumerate() {
            let matrix = match skinned {
                true => placing.blend(vertices.joints[index], vertices.weights[index].to_array()),
                false => placing.world,
            };
            self.keys.push(VertexKey::new(node, index as u32));
            self.points.push(matrix.transform_point3(*position));
            let normal = vertices.normals.get(index).copied().unwrap_or(Vec3::ZERO);
            self.normals
                .push(matrix.transform_vector3(normal).normalize_or_zero());
        }
        let count = positions.len() as u32;
        for corners in mesh.indices.as_chunks::<3>().0 {
            if corners.iter().all(|corner| *corner < count) {
                self.triangles.push(corners.map(|corner| corner + first));
            }
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.points.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    #[must_use]
    pub fn keys(&self) -> &[VertexKey] {
        &self.keys
    }

    #[must_use]
    pub fn points(&self) -> &[Vec3] {
        &self.points
    }

    #[must_use]
    pub fn normals(&self) -> &[Vec3] {
        &self.normals
    }

    #[must_use]
    pub fn cast(&self, origin: Vec3, direction: Vec3) -> Option<Hit> {
        let ray = Ray { origin, direction };
        self.triangles
            .iter()
            .filter_map(|corners| {
                ray.crossing(corners.map(|corner| self.points[corner as usize]))
                    .map(|crossing| crossing.along)
            })
            .min_by(f32::total_cmp)
            .map(|distance| Hit {
                point: ray.at(distance),
                distance,
            })
    }

    pub fn vertices_near(
        &self,
        centre: Vec3,
        radius: f32,
    ) -> impl Iterator<Item = (usize, f32)> + '_ {
        let reach = radius.max(f32::EPSILON);
        self.points
            .iter()
            .enumerate()
            .filter_map(move |(index, point)| {
                let away = point.distance(centre) / reach;
                (away <= 1.0).then(|| (index, falloff(away)))
            })
    }
}

fn falloff(away: f32) -> f32 {
    let falloff = 1.0 - away * away;
    falloff * falloff
}

struct Placing {
    world: Mat4,
    palette: Vec<Mat4>,
    morphs: Vec<f32>,
}

impl Placing {
    fn blend(&self, joints: [u16; 4], weights: [f32; 4]) -> Mat4 {
        let mut matrix = Mat4::ZERO;
        let mut total = 0.0;
        for (joint, weight) in joints.iter().zip(weights) {
            let Some(palette) = self.palette.get(usize::from(*joint)) else {
                continue;
            };
            if weight > 0.0 {
                matrix += *palette * weight;
                total += weight;
            }
        }
        match total > 0.0 {
            true => matrix * total.recip(),
            false => self.world,
        }
    }
}
