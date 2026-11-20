use rikka_model_core::glam::{IVec3, Vec3};
use rustc_hash::FxHashMap;

use crate::surface::Surface;

const CELL: f32 = 0.004;
const MATCH: f32 = 0.002;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mirror {
    twins: Vec<Option<u32>>,
}

impl Mirror {
    #[must_use]
    pub fn from_surface(rest: &Surface) -> Mirror {
        let mut cells: FxHashMap<IVec3, Vec<u32>> = FxHashMap::default();
        for (index, point) in rest.points().iter().enumerate() {
            cells.entry(cell(*point)).or_default().push(index as u32);
        }
        let twins = rest
            .points()
            .iter()
            .enumerate()
            .map(|(index, point)| {
                let mirrored = Vec3::new(-point.x, point.y, point.z);
                let normal = rest.normals()[index];
                let desired = Vec3::new(-normal.x, normal.y, normal.z);
                let centre = cell(mirrored);
                neighbours(centre)
                    .filter_map(|at| cells.get(&at))
                    .flatten()
                    .copied()
                    .filter(|other| rest.points()[*other as usize].distance(mirrored) <= MATCH)
                    .min_by(|one, two| {
                        let score = |other: u32| {
                            rest.points()[other as usize].distance(mirrored)
                                - rest.normals()[other as usize].dot(desired) * MATCH
                        };
                        score(*one).total_cmp(&score(*two)).then(one.cmp(two))
                    })
            })
            .collect();
        Mirror { twins }
    }

    #[must_use]
    pub fn twin(&self, index: usize) -> Option<usize> {
        self.twins
            .get(index)
            .copied()
            .flatten()
            .map(|twin| twin as usize)
    }
}

fn cell(point: Vec3) -> IVec3 {
    (point / CELL).floor().as_ivec3()
}

fn neighbours(centre: IVec3) -> impl Iterator<Item = IVec3> {
    (-1..=1).flat_map(move |x| {
        (-1..=1).flat_map(move |y| (-1..=1).map(move |z| centre + IVec3::new(x, y, z)))
    })
}
