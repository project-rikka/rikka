use std::ops::{AddAssign, SubAssign};

#[derive(Debug, Default)]
pub(crate) struct Sums<T> {
    tree: Vec<T>,
}

impl<T: Copy + Default + AddAssign + SubAssign> Sums<T> {
    pub(crate) fn from_values(values: impl IntoIterator<Item = T>) -> Self {
        let mut tree: Vec<T> = values.into_iter().collect();
        for place in 1..=tree.len() {
            let parent = place + place.isolate_lowest_one();
            if parent <= tree.len() {
                let below = tree[place - 1];
                tree[parent - 1] += below;
            }
        }
        Self { tree }
    }

    pub(crate) fn add(&mut self, index: usize, amount: T) {
        self.each_above(index, |sum| *sum += amount);
    }

    pub(crate) fn subtract(&mut self, index: usize, amount: T) {
        self.each_above(index, |sum| *sum -= amount);
    }

    pub(crate) fn before(&self, index: usize) -> T {
        let mut place = index.min(self.tree.len());
        let mut sum = T::default();
        while place > 0 {
            sum += self.tree[place - 1];
            place -= place.isolate_lowest_one();
        }
        sum
    }

    pub(crate) fn find(&self, mut takes: impl FnMut(T) -> bool) -> usize {
        let mut place = 0;
        let mut step = self.tree.len().next_power_of_two();
        while step > 0 {
            let next = place + step;
            if next <= self.tree.len() && takes(self.tree[next - 1]) {
                place = next;
            }
            step /= 2;
        }
        place
    }

    fn each_above(&mut self, index: usize, mut change: impl FnMut(&mut T)) {
        let mut place = index + 1;
        while place <= self.tree.len() {
            change(&mut self.tree[place - 1]);
            place += place.isolate_lowest_one();
        }
    }
}
