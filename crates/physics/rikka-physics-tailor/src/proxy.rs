use glam::Vec3;
use rikka_model_core::{INFLUENCES, NodeId};

use crate::asset::{EdgeKind, StrandNode};
use crate::figure::Figure;

pub(crate) const UNMAPPED: u32 = u32::MAX;

pub(crate) type NodeWeights = Vec<(u32, f32)>;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Weights {
    pairs: Vec<(NodeId, f32)>,
}

impl Weights {
    pub(crate) fn add(&mut self, node: NodeId, weight: f32) {
        if weight <= 0.0 || !weight.is_finite() {
            return;
        }
        match self.pairs.binary_search_by_key(&node, |(key, _)| *key) {
            Ok(at) => self.pairs[at].1 += weight,
            Err(at) => self.pairs.insert(at, (node, weight)),
        }
    }

    pub(crate) fn halves(first: &Weights, second: &Weights) -> Weights {
        let mut blended = Weights::default();
        for weights in [first, second] {
            let total: f32 = weights.pairs.iter().map(|(_, weight)| weight).sum();
            if total <= 0.0 {
                continue;
            }
            for (node, weight) in &weights.pairs {
                blended.add(*node, weight / total);
            }
        }
        blended
    }

    pub(crate) fn top(&self) -> Option<([NodeId; INFLUENCES], [f32; INFLUENCES])> {
        let mut sorted = self.pairs.clone();
        sorted.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        sorted.truncate(INFLUENCES);
        let total: f32 = sorted.iter().map(|(_, weight)| weight).sum();
        if total <= 1e-6 {
            return None;
        }
        let strongest = sorted[0].0;
        let joints =
            std::array::from_fn(|slot| sorted.get(slot).map_or(strongest, |(node, _)| *node));
        let weights =
            std::array::from_fn(|slot| sorted.get(slot).map_or(0.0, |(_, weight)| weight / total));
        Some((joints, weights))
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Proxy {
    pub(crate) positions: Vec<Vec3>,
    pub(crate) pinned: Vec<bool>,
    pub(crate) weights: Vec<Weights>,
    pub(crate) radius: Vec<f32>,
    pub(crate) area: Vec<f32>,
    pub(crate) spread: Vec<f32>,
    pub(crate) triangles: Vec<[u32; 3]>,
    pub(crate) edges: Vec<(u32, u32, EdgeKind)>,
    pub(crate) vertex_map: Vec<(u32, u32)>,
    pub(crate) unmapped: Vec<u32>,
    pub(crate) strand_nodes: Vec<StrandNode>,
    pub(crate) node_weights: Vec<NodeWeights>,
    pub(crate) cross_links: Vec<(u32, u32)>,
}

impl Proxy {
    pub(crate) fn len(&self) -> usize {
        self.positions.len()
    }

    pub(crate) fn gather(&mut self, figure: &Figure<'_>, spreads: &[f32]) {
        let count = self.positions.len();
        self.weights = vec![Weights::default(); count];
        self.area = vec![0.0; count];
        let mut spread = vec![(0.0f32, 0u32); count];
        let mut target_of = vec![UNMAPPED; figure.vertices.len()];
        for (vertex, target) in &self.vertex_map {
            target_of[*vertex as usize] = *target;
            if *target == UNMAPPED {
                continue;
            }
            let rest = &figure.vertices[*vertex as usize];
            if rest.weighted {
                for (joint, weight) in rest.joints.iter().zip(rest.weights) {
                    self.weights[*target as usize].add(*joint, weight);
                }
            }
            let sum = &mut spread[*target as usize];
            sum.0 += spreads[*vertex as usize];
            sum.1 += 1;
        }
        for triangle in &figure.triangles {
            let vertex_map = triangle.corners.map(|corner| target_of[corner as usize]);
            if vertex_map.contains(&UNMAPPED) {
                continue;
            }
            let share = figure.triangle_area(triangle) / 3.0;
            for target in vertex_map {
                self.area[target as usize] += share;
            }
        }
        self.spread = spread
            .into_iter()
            .map(|(sum, count)| match count {
                0 => 1.0,
                count => sum / count as f32,
            })
            .collect();
    }

    pub(crate) fn blend(&mut self, blended: &[(u32, [u32; 2])]) {
        for (vertex, [a, b]) in blended {
            let (a, b, vertex) = (*a as usize, *b as usize, *vertex as usize);
            self.weights[vertex] = Weights::halves(&self.weights[a], &self.weights[b]);
            self.spread[vertex] = (self.spread[a] + self.spread[b]) * 0.5;
        }
    }

    pub(crate) fn spread_mass(&mut self) {
        let context: f32 = self.area.iter().sum();
        let mut shares = vec![0.0f32; self.positions.len()];
        for triangle in &self.triangles {
            let [a, b, c] = triangle.map(|corner| self.positions[corner as usize]);
            let share = (b - a).cross(c - a).length() / 6.0;
            for corner in triangle {
                shares[*corner as usize] += share;
            }
        }
        let surface: f32 = shares.iter().sum();
        if surface <= 0.0 || context <= 0.0 {
            return;
        }
        self.area = shares
            .into_iter()
            .map(|share| share * context / surface)
            .collect();
    }
}
