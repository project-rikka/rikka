use crate::body::BodyId;
use crate::body::mesh::BodyMesh;

#[derive(Clone, Debug)]
pub(crate) struct Region {
    pub(crate) body: BodyId,
    pub(crate) triangles: Vec<u32>,
    pub(crate) cumulative: Vec<f32>,
}

impl Region {
    pub(crate) fn new(body: BodyId, mesh: &BodyMesh, triangles: Vec<u32>) -> Region {
        let mut total = 0.0;
        let cumulative = triangles
            .iter()
            .map(|&triangle| {
                total += mesh.areas[triangle as usize].max(1e-12);
                total
            })
            .collect();
        Region {
            body,
            triangles,
            cumulative,
        }
    }

    pub(crate) fn pick(&self, unit: f32) -> u32 {
        let total = self.cumulative.last().copied().unwrap_or(0.0);
        let target = unit * total;
        let at = self.cumulative.partition_point(|&sum| sum <= target);
        self.triangles[at.min(self.triangles.len() - 1)]
    }
}
