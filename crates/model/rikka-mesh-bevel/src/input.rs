use rikka_mesh_edit::{EdgeId, EditMesh, IdSet, VertId};
use rikka_polymesh::{BEVEL_WEIGHT_EDGE, BEVEL_WEIGHT_VERT, Domain};

use crate::error::{Error, Result};
use crate::options::Affect;
use crate::weights::WeightTable;

const ANGLE_LIMIT_BIAS: f32 = 0.000_000_175;
const GROUP_THRESHOLD: f32 = 0.5;
const DEFAULT_ANGLE_LIMIT: f32 = 30.0 * (std::f64::consts::PI / 180.0) as f32;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WeightLayers {
    pub vert: String,
    pub edge: String,
}

impl Default for WeightLayers {
    fn default() -> WeightLayers {
        WeightLayers {
            vert: BEVEL_WEIGHT_VERT.to_owned(),
            edge: BEVEL_WEIGHT_EDGE.to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Limit {
    None,
    Angle(f32),
    Weight,
    VertexGroup { group: u32, invert: bool },
}

impl Default for Limit {
    fn default() -> Limit {
        Limit::Angle(DEFAULT_ANGLE_LIMIT)
    }
}

#[derive(Clone, Debug, Default)]
pub struct BevelInput {
    pub verts: IdSet<VertId>,
    pub edges: IdSet<EdgeId>,
    pub weights: Option<WeightLayers>,
    pub vertex_group: Option<u32>,
}

impl BevelInput {
    #[must_use]
    pub fn from_selection(mesh: &EditMesh) -> BevelInput {
        let mut input = BevelInput::default();
        input.verts.extend(
            mesh.verts()
                .filter(|&vert| mesh.is_vert_selected(vert) && !mesh.is_vert_hidden(vert)),
        );
        input.edges.extend(
            mesh.edges()
                .filter(|&edge| mesh.is_edge_selected(edge) && !mesh.is_edge_hidden(edge)),
        );
        input
    }

    #[must_use]
    pub fn from_edges(mesh: &EditMesh, edges: impl IntoIterator<Item = EdgeId>) -> BevelInput {
        let mut input = BevelInput::default();
        for edge in edges {
            input.edges.insert(edge);
            input.verts.extend(mesh.edge_verts(edge));
        }
        input
    }

    #[must_use]
    pub fn from_verts(verts: impl IntoIterator<Item = VertId>) -> BevelInput {
        let mut input = BevelInput::default();
        input.verts.extend(verts);
        input
    }

    pub fn from_limit(
        mesh: &EditMesh,
        affect: Affect,
        limit: Limit,
        layers: &WeightLayers,
    ) -> Result<BevelInput> {
        if let Limit::VertexGroup { group, .. } = limit
            && group as usize >= mesh.vertex_groups().len()
        {
            return Err(Error::UnknownVertexGroup(group));
        }
        let group_weight = |vert: VertId| match limit {
            Limit::VertexGroup { group, invert } => {
                let weight = mesh.vert_weight(vert, group);
                match invert {
                    true => 1.0 - weight,
                    false => weight,
                }
            }
            _ => 1.0,
        };
        let mut input = BevelInput {
            weights: matches!(limit, Limit::Weight).then(|| layers.clone()),
            vertex_group: match limit {
                Limit::VertexGroup { group, .. } => Some(group),
                _ => None,
            },
            ..BevelInput::default()
        };
        match (affect, limit) {
            (Affect::Vertices, _) => {
                let weights = WeightTable::resolve(mesh, &layers.vert, Domain::Point);
                for vert in mesh.verts() {
                    let selected = match limit {
                        Limit::Weight => weights.get(vert.index()) != 0.0,
                        _ => group_weight(vert) >= GROUP_THRESHOLD,
                    };
                    if selected {
                        input.verts.insert(vert);
                    }
                }
            }
            (Affect::Edges, Limit::Angle(angle)) => {
                let threshold = (angle + ANGLE_LIMIT_BIAS).cos();
                for edge in mesh.edges() {
                    let Some((a, b)) = mesh.edge_loop_pair(edge) else {
                        continue;
                    };
                    let normal_a = mesh.face_normal(mesh.loop_face(a));
                    let normal_b = mesh.face_normal(mesh.loop_face(b));
                    if normal_a.dot(normal_b) < threshold {
                        input.add_edge(mesh, edge);
                    }
                }
            }
            (Affect::Edges, _) => {
                let weights = WeightTable::resolve(mesh, &layers.edge, Domain::Edge);
                for edge in mesh.edges() {
                    if !mesh.is_edge_manifold(edge) {
                        continue;
                    }
                    let selected = match limit {
                        Limit::Weight => weights.get(edge.index()) != 0.0,
                        _ => {
                            let [a, b] = mesh.edge_verts(edge);
                            group_weight(a) >= GROUP_THRESHOLD && group_weight(b) >= GROUP_THRESHOLD
                        }
                    };
                    if selected {
                        input.add_edge(mesh, edge);
                    }
                }
            }
        }
        Ok(input)
    }

    fn add_edge(&mut self, mesh: &EditMesh, edge: EdgeId) {
        self.edges.insert(edge);
        self.verts.extend(mesh.edge_verts(edge));
    }
}
