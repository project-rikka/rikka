use std::cmp::Ordering;

use glam::DVec2;

pub(crate) trait Kernel {
    type Support: Clone;

    fn vertex_count(&self) -> usize;

    fn approx(&self, vertex: u32) -> DVec2;

    fn orient(&self, a: u32, b: u32, c: u32) -> i32;

    fn in_circle(&self, a: u32, b: u32, c: u32, d: u32) -> i32;

    fn compare(&self, a: u32, b: u32) -> Ordering;

    fn intersection(&mut self, first: &Self::Support, second: &Self::Support) -> u32;

    fn equal(&self, a: u32, b: u32) -> bool {
        a == b || self.compare(a, b) == Ordering::Equal
    }

    fn between(&self, a: u32, b: u32, c: u32) -> bool {
        let first = self.compare(a, c);
        first != Ordering::Equal && first == self.compare(c, b)
    }
}
