use std::collections::HashMap;

use glam::{IVec3, Vec3};

#[derive(Clone, Debug)]
pub(crate) struct PointGrid {
    cell: f32,
    cells: HashMap<IVec3, (u32, u32)>,
    items: Vec<u32>,
}

impl PointGrid {
    pub(crate) fn new(points: impl IntoIterator<Item = (u32, Vec3)>, cell: f32) -> PointGrid {
        let cell = cell.max(1e-6);
        let mut keyed: Vec<(IVec3, u32)> = points
            .into_iter()
            .filter(|(_, point)| point.is_finite())
            .map(|(item, point)| (cell_of(point, cell), item))
            .collect();
        keyed.sort_by(|a, b| a.0.to_array().cmp(&b.0.to_array()).then(a.1.cmp(&b.1)));
        let mut cells = HashMap::new();
        let mut start = 0;
        while start < keyed.len() {
            let key = keyed[start].0;
            let end = start + keyed[start..].partition_point(|(other, _)| *other == key);
            cells.insert(key, (start as u32, end as u32));
            start = end;
        }
        PointGrid {
            cell,
            cells,
            items: keyed.into_iter().map(|(_, item)| item).collect(),
        }
    }

    pub(crate) fn near(&self, point: Vec3, radius: f32) -> impl Iterator<Item = u32> + '_ {
        let low = cell_of(point - radius, self.cell);
        let high = cell_of(point + radius, self.cell);
        (low.x..=high.x).flat_map(move |x| {
            (low.y..=high.y).flat_map(move |y| {
                (low.z..=high.z).flat_map(move |z| {
                    let range = self
                        .cells
                        .get(&IVec3::new(x, y, z))
                        .map_or(0..0, |(start, end)| *start as usize..*end as usize);
                    self.items[range].iter().copied()
                })
            })
        })
    }

    pub(crate) fn nearest(&self, point: Vec3, position: impl Fn(u32) -> Vec3) -> Option<u32> {
        let closest = |items: &mut dyn Iterator<Item = u32>, limit: f32| {
            items
                .map(|item| (position(item).distance(point), item))
                .filter(|(distance, _)| *distance <= limit)
                .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
                .map(|(_, item)| item)
        };
        for reach in [self.cell, self.cell * 3.0] {
            if let Some(item) = closest(&mut self.near(point, reach), reach) {
                return Some(item);
            }
        }
        closest(&mut self.items.iter().copied(), f32::INFINITY)
    }
}

fn cell_of(point: Vec3, cell: f32) -> IVec3 {
    (point / cell).floor().as_ivec3()
}
