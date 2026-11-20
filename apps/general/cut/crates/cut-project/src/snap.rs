#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snap {
    points: Vec<i64>,
}

impl Snap {
    #[must_use]
    pub fn new(points: impl IntoIterator<Item = i64>) -> Snap {
        let mut points: Vec<i64> = points.into_iter().collect();
        points.sort_unstable();
        points.dedup();
        Snap { points }
    }

    #[must_use]
    pub fn pull(&self, edges: &[i64], within: i64) -> Option<i64> {
        edges
            .iter()
            .filter_map(|edge| self.nearest(*edge).map(|point| point - edge))
            .filter(|pull| pull.abs() <= within)
            .min_by_key(|pull| pull.abs())
    }

    fn nearest(&self, frame: i64) -> Option<i64> {
        let after = self.points.partition_point(|point| *point < frame);
        let above = self.points.get(after).copied();
        let below = after.checked_sub(1).map(|at| self.points[at]);
        match (below, above) {
            (Some(below), Some(above)) if frame - below <= above - frame => Some(below),
            (_, Some(above)) => Some(above),
            (below, None) => below,
        }
    }
}
