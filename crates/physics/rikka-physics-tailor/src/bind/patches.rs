use crate::figure::Figure;
use crate::geometry::{PointGrid, Union};
use crate::proxy::Proxy;

const MAPPED_SHARE: f32 = 0.5;

pub(super) struct Patches {
    of_triangle: Vec<u32>,
    allowed: Vec<Vec<u32>>,
    component: Vec<u32>,
}

impl Patches {
    pub(super) fn new(figure: &Figure<'_>, proxy: &Proxy) -> Patches {
        let mut sewn = Union::new(proxy.len());
        for [a, b, c] in &proxy.triangles {
            sewn.join(*a, *b);
            sewn.join(*a, *c);
        }
        let (patch, _) = sewn.labels();
        let of_triangle = proxy
            .triangles
            .iter()
            .map(|triangle| patch[triangle[0] as usize])
            .collect();
        let (component, count) = drawn_components(figure, proxy);
        let mut mapped: Vec<Vec<u32>> = vec![Vec::new(); count];
        let mut sizes = vec![0usize; count];
        for ((vertex, target), component) in proxy.vertex_map.iter().zip(&component) {
            sizes[*component as usize] += 1;
            if proxy.unmapped.binary_search(vertex).is_err() {
                mapped[*component as usize].push(patch[*target as usize]);
            }
        }
        let allowed = mapped
            .into_iter()
            .zip(sizes)
            .map(|(mut patches, size)| {
                if (patches.len() as f32) < MAPPED_SHARE * size as f32 {
                    return Vec::new();
                }
                patches.sort_unstable();
                patches.dedup();
                patches
            })
            .collect();
        Patches {
            of_triangle,
            allowed,
            component,
        }
    }

    pub(super) fn admits(&self, target: usize, triangle: u32) -> bool {
        let allowed = &self.allowed[self.component[target] as usize];
        allowed.is_empty()
            || allowed
                .binary_search(&self.of_triangle[triangle as usize])
                .is_ok()
    }

    pub(super) fn component(&self, target: usize) -> u32 {
        self.component[target]
    }

    pub(super) fn count(&self) -> usize {
        self.allowed.len()
    }
}

fn drawn_components(figure: &Figure<'_>, proxy: &Proxy) -> (Vec<u32>, usize) {
    let mut slot = vec![u32::MAX; figure.vertices.len()];
    for (at, (vertex, _)) in proxy.vertex_map.iter().enumerate() {
        slot[*vertex as usize] = at as u32;
    }
    let mut joined = Union::new(proxy.vertex_map.len());
    for triangle in &figure.triangles {
        let corners = triangle.corners.map(|corner| slot[corner as usize]);
        if corners.contains(&u32::MAX) {
            continue;
        }
        joined.join(corners[0], corners[1]);
        joined.join(corners[0], corners[2]);
    }
    let same = figure.same_distance();
    let position = |at: u32| figure.vertices[proxy.vertex_map[at as usize].0 as usize].position;
    let grid = PointGrid::new(
        (0..proxy.vertex_map.len() as u32).map(|at| (at, position(at))),
        same * 2.0,
    );
    for at in 0..proxy.vertex_map.len() as u32 {
        for other in grid.near(position(at), same) {
            if other > at && position(other).distance(position(at)) <= same {
                joined.join(at, other);
            }
        }
    }
    joined.labels()
}
