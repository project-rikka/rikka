use glam::Vec3;

use crate::figure::Figure;
use crate::geometry::{Feature, TriangleTree, doubled_area_normal};
use crate::proxy::Proxy;

const OVER: usize = 16;

pub(crate) struct Relief {
    pub(crate) relief_height: Vec<f32>,
    pub(crate) relief_depth: Vec<f32>,
}

impl Relief {
    pub(crate) fn measure(
        figure: &Figure<'_>,
        proxy: &Proxy,
        triangles: &TriangleTree,
        limit: f32,
    ) -> Relief {
        let faces: Vec<Vec3> = (0..proxy.triangles.len() as u32)
            .map(|triangle| doubled_area_normal(triangles.corners(triangle)).normalize_or_zero())
            .collect();
        let mut raised = vec![0.0f32; proxy.triangles.len()];
        let mut sunk = vec![0.0f32; proxy.triangles.len()];
        let mut measure = |sample: Vec3| {
            for hit in triangles.nearest_few_where(sample, OVER, limit, |_| true) {
                if !matches!(hit.closest.feature, Feature::Face) {
                    continue;
                }
                let triangle = hit.triangle as usize;
                let height = (sample - hit.closest.point).dot(faces[triangle]);
                raised[triangle] = raised[triangle].max(height);
                sunk[triangle] = sunk[triangle].max(-height);
            }
        };
        let mut drawn = vec![false; figure.vertices.len()];
        for &(vertex, _) in &proxy.vertex_map {
            drawn[vertex as usize] = true;
            measure(figure.vertices[vertex as usize].position);
        }
        for triangle in &figure.triangles {
            if !triangle
                .corners
                .iter()
                .all(|&corner| drawn[corner as usize])
            {
                continue;
            }
            let [a, b, c] = triangle
                .corners
                .map(|corner| figure.vertices[corner as usize].position);
            for sample in [
                (a + b + c) / 3.0,
                (a + b) * 0.5,
                (b + c) * 0.5,
                (c + a) * 0.5,
            ] {
                measure(sample);
            }
        }
        let mut relief = Relief {
            relief_height: vec![0.0; proxy.len()],
            relief_depth: vec![0.0; proxy.len()],
        };
        for (triangle, triangle_corners) in proxy.triangles.iter().enumerate() {
            for &corner in triangle_corners {
                let corner = corner as usize;
                relief.relief_height[corner] = relief.relief_height[corner].max(raised[triangle]);
                relief.relief_depth[corner] = relief.relief_depth[corner].max(sunk[triangle]);
            }
        }
        relief
    }
}
