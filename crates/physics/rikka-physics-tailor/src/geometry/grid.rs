use glam::{IVec3, Vec3};
use rustc_hash::FxHashMap;

#[derive(Clone, Debug)]
pub(crate) struct PointGrid {
    cell: f32,
    cells: FxHashMap<IVec3, (u32, u32)>,
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
        keyed.sort_unstable_by(|a, b| a.0.to_array().cmp(&b.0.to_array()).then(a.1.cmp(&b.1)));
        let mut cells = FxHashMap::default();
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

    pub(crate) fn near(&self, point: Vec3, radius: f32) -> Near<'_> {
        let low = cell_of(point - radius, self.cell);
        let high = cell_of(point + radius, self.cell);
        Near {
            grid: self,
            low,
            high,
            items: [].iter(),
            next: low.cmple(high).all().then_some(low),
        }
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

    fn items_in(&self, cell: IVec3) -> &[u32] {
        self.cells.get(&cell).map_or(&[], |(start, end)| {
            &self.items[*start as usize..*end as usize]
        })
    }
}

pub(crate) struct Near<'a> {
    grid: &'a PointGrid,
    low: IVec3,
    high: IVec3,
    items: std::slice::Iter<'a, u32>,
    next: Option<IVec3>,
}

impl Near<'_> {
    fn after(&self, mut cell: IVec3) -> Option<IVec3> {
        if cell.z < self.high.z {
            cell.z += 1;
            return Some(cell);
        }
        cell.z = self.low.z;
        if cell.y < self.high.y {
            cell.y += 1;
            return Some(cell);
        }
        cell.y = self.low.y;
        if cell.x < self.high.x {
            cell.x += 1;
            return Some(cell);
        }
        None
    }
}

impl Iterator for Near<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        loop {
            if let Some(item) = self.items.next() {
                return Some(*item);
            }
            let cell = self.next?;
            self.items = self.grid.items_in(cell).iter();
            self.next = self.after(cell);
        }
    }
}

fn cell_of(point: Vec3, cell: f32) -> IVec3 {
    (point / cell).floor().as_ivec3()
}
