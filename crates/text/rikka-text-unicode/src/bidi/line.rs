use std::ops::Range;

use super::analysis::Bidi;
use super::classes::{SEPARATORS, TRAILING_WHITESPACE};
use super::level::Level;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VisualRun {
    pub range: Range<usize>,
    pub level: Level,
}

#[derive(Clone, Debug, Default)]
pub struct VisualLine {
    levels: Vec<Level>,
    runs: Vec<VisualRun>,
    character_levels: Vec<u8>,
}

impl VisualLine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn levels(&self) -> &[Level] {
        &self.levels
    }

    pub fn runs(&self) -> &[VisualRun] {
        &self.runs
    }
}

impl Bidi {
    pub fn reorder_line(&self, line: Range<usize>, visual: &mut VisualLine) {
        visual.levels.clear();
        visual.runs.clear();
        visual.character_levels.clear();
        let end = line.end.min(self.text_len());
        if self.is_uniform() {
            if line.start < end {
                visual.levels.resize(end - line.start, Level::LEFT_TO_RIGHT);
                visual.runs.push(VisualRun {
                    range: line.start..end,
                    level: Level::LEFT_TO_RIGHT,
                });
            }
            return;
        }
        let first = self.offsets.partition_point(|&offset| offset < line.start);
        let last = self.offsets.partition_point(|&offset| offset < end);
        if first >= last {
            return;
        }
        let paragraph_level = self.paragraph_level(self.offsets[first]);
        visual
            .character_levels
            .extend_from_slice(&self.resolved[first..last]);
        let mut trailing = true;
        for (level, &class) in visual
            .character_levels
            .iter_mut()
            .zip(&self.original[first..last])
            .rev()
        {
            if SEPARATORS.has(class) {
                *level = paragraph_level;
                trailing = true;
            } else if trailing && TRAILING_WHITESPACE.has(class) {
                *level = paragraph_level;
            } else {
                trailing = false;
            }
        }
        let start = self.offsets[first];
        for (position, &level) in visual.character_levels.iter().enumerate() {
            let index = first + position;
            let from = self.offsets[index];
            let to = self
                .offsets
                .get(index + 1)
                .copied()
                .unwrap_or(self.text_len())
                .min(end);
            let level = Level::from_resolved(level);
            visual.levels.extend(std::iter::repeat_n(level, to - from));
            match visual.runs.last_mut() {
                Some(run) if run.level == level => run.range.end = to,
                _ => visual.runs.push(VisualRun {
                    range: from..to,
                    level,
                }),
            }
        }
        debug_assert_eq!(visual.levels.len(), end - start);
        reorder_runs(&mut visual.runs);
    }
}

fn reorder_runs(runs: &mut [VisualRun]) {
    let numbers = || runs.iter().map(|run| run.level.number());
    let (Some(highest), Some(lowest_odd)) = (
        numbers().max(),
        numbers().filter(|level| level & 1 == 1).min(),
    ) else {
        return;
    };
    for level in (lowest_odd..=highest).rev() {
        let mut index = 0;
        while index < runs.len() {
            if runs[index].level.number() < level {
                index += 1;
                continue;
            }
            let start = index;
            while index < runs.len() && runs[index].level.number() >= level {
                index += 1;
            }
            runs[start..index].reverse();
        }
    }
}
