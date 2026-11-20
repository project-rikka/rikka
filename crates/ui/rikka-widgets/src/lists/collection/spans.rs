use std::ops::{AddAssign, Range, SubAssign};

use crate::sums::Sums;

const OVERSCAN: f32 = 0.5;

#[derive(Debug, Default)]
pub(crate) struct Spans {
    heights: Vec<Option<f32>>,
    index: Index,
    estimate: f32,
    average: f32,
    measured: f64,
    counted: usize,
    fixed: Option<f32>,
}

impl Spans {
    pub(crate) fn adopt(&mut self, count: usize, estimate: f32, fixed: Option<f32>) {
        self.estimate = estimate.max(1.0);
        if self.fixed != fixed {
            self.fixed = fixed;
            self.heights.clear();
        }
        if self.heights.len() != count {
            self.heights.resize(count, None);
            self.rebuild();
        }
    }

    pub(crate) fn remap(&mut self, count: usize, from: impl Fn(usize) -> Option<usize>) {
        let heights = (0..count)
            .map(|band| from(band).and_then(|was| self.heights.get(was).copied().flatten()))
            .collect();
        self.heights = heights;
        self.rebuild();
    }

    pub(crate) fn measures(&self) -> bool {
        self.fixed.is_none()
    }

    pub(crate) fn forget(&mut self) {
        if !self.measures() {
            return;
        }
        self.heights.iter_mut().for_each(|height| *height = None);
        self.rebuild();
    }
    pub(crate) fn measured(&mut self, index: usize, height: f32) -> bool {
        if !self.measures() {
            return false;
        }
        let Some(slot) = self.heights.get_mut(index) else {
            return false;
        };
        if *slot == Some(height) {
            return false;
        }
        let previous = slot.replace(height);
        match previous {
            Some(was) => {
                self.measured += f64::from(height) - f64::from(was);
                self.index
                    .measure(index, f64::from(height) - f64::from(was), 0);
            }
            None => {
                self.measured += f64::from(height);
                self.counted += 1;
                self.index.measure(index, f64::from(height), -1);
            }
        }
        self.settle_average();
        true
    }

    pub(crate) fn count(&self) -> usize {
        self.heights.len()
    }

    pub(crate) fn top(&self, index: usize) -> f32 {
        match self.fixed {
            Some(height) => index.min(self.count()) as f32 * height,
            None => self.index.top(index, f64::from(self.average)) as f32,
        }
    }

    pub(crate) fn height(&self, index: usize) -> f32 {
        self.top(index + 1) - self.top(index)
    }

    pub(crate) fn content(&self) -> f32 {
        self.top(self.count())
    }

    pub(crate) fn page(&self, room: f32) -> usize {
        let step = match self.fixed {
            Some(height) => height,
            None => self.average.max(1.0),
        };
        ((room / step.max(1.0)).floor() as usize).max(1)
    }

    pub(crate) fn visible(&self, offset: f32, room: f32) -> Range<usize> {
        let count = self.count();
        if count == 0 {
            return 0..0;
        }
        let start = self.at(offset);
        let end = self.at(offset + room.max(0.0)) + 1;
        start..end.min(count).max(start)
    }

    pub(crate) fn visible_range(&self, offset: f32, room: f32) -> Range<usize> {
        let room = room.max(0.0);
        let overscan = room * OVERSCAN;
        self.visible((offset - overscan).max(0.0), room + overscan * 2.0)
    }

    pub(crate) fn at(&self, down: f32) -> usize {
        let last = self.count().saturating_sub(1);
        match self.fixed {
            Some(height) => ((down / height.max(1.0)).floor().max(0.0) as usize).min(last),
            None => self
                .index
                .at(f64::from(down.max(0.0)), f64::from(self.average))
                .min(last),
        }
    }

    fn settle_average(&mut self) {
        if self.counted > 0 {
            self.average = (self.measured / self.counted as f64) as f32;
        } else if self.average <= 0.0 {
            self.average = self.estimate;
        }
    }

    fn rebuild(&mut self) {
        self.measured = self
            .heights
            .iter()
            .flatten()
            .map(|&height| f64::from(height))
            .sum();
        self.counted = self.heights.iter().flatten().count();
        self.settle_average();
        self.index.rebuild(&self.heights);
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Weighed {
    measured: f64,
    guessed: i64,
}

impl Weighed {
    fn height(self, average: f64) -> f64 {
        self.measured + self.guessed as f64 * average
    }
}

impl AddAssign for Weighed {
    fn add_assign(&mut self, other: Weighed) {
        self.measured += other.measured;
        self.guessed += other.guessed;
    }
}

impl SubAssign for Weighed {
    fn sub_assign(&mut self, other: Weighed) {
        self.measured -= other.measured;
        self.guessed -= other.guessed;
    }
}

#[derive(Debug, Default)]
struct Index {
    sums: Sums<Weighed>,
}

impl Index {
    fn rebuild(&mut self, heights: &[Option<f32>]) {
        self.sums = Sums::from_values(heights.iter().map(|height| Weighed {
            measured: height.map_or(0.0, f64::from),
            guessed: i64::from(height.is_none()),
        }));
    }

    fn measure(&mut self, index: usize, height: f64, guessed: i64) {
        self.sums.add(
            index,
            Weighed {
                measured: height,
                guessed,
            },
        );
    }

    fn top(&self, index: usize, average: f64) -> f64 {
        self.sums.before(index).height(average)
    }

    fn at(&self, down: f64, average: f64) -> usize {
        let mut left = down;
        self.sums.find(|span| {
            let height = span.height(average);
            let fits = height <= left;
            if fits {
                left -= height;
            }
            fits
        })
    }
}
