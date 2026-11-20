use std::ops::Range;

use rikka_text_unicode::{WordRule, WordSegment, WordSegments};

use crate::editing::cursor::Affinity;
use crate::editing::window::Window;

const CONTEXT_SEGMENTS: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Space,
    Word,
    Symbol,
}

#[derive(Clone, Debug)]
struct Run {
    range: Range<usize>,
    class: Class,
}

pub(crate) struct Runs {
    runs: Vec<Run>,
    trusted: Range<usize>,
}

impl Runs {
    pub(crate) fn new(window: &Window<'_>, rule: WordRule) -> Option<Runs> {
        let text = window.text();
        let mut starts = Vec::new();
        let mut runs: Vec<Run> = Vec::new();
        for segment in WordSegments::new(text, rule) {
            starts.push(segment.start);
            let class = class_of(&segment);
            let end = segment.start + segment.text.len();
            match runs.last_mut() {
                Some(run) if joins(run.class, class) => run.range.end = end,
                _ => runs.push(Run {
                    range: segment.start..end,
                    class,
                }),
            }
        }
        starts.push(text.len());
        let segments = starts.len() - 1;
        let start = match window.is_text_start() {
            true => 0,
            false => *starts.get(CONTEXT_SEGMENTS)?,
        };
        let end = match window.is_text_end() {
            true => text.len(),
            false => starts[segments.checked_sub(CONTEXT_SEGMENTS)?],
        };
        (start <= end).then_some(Runs {
            runs,
            trusted: start..end,
        })
    }

    pub(crate) fn next(&self, at: usize) -> Option<usize> {
        if at < self.trusted.start {
            return None;
        }
        let end = match self.containing(at) {
            None => at,
            Some(index) => {
                let run = &self.runs[index];
                match (run.class, self.runs.get(index + 1)) {
                    (Class::Word | Class::Symbol, Some(next)) if next.class == Class::Space => {
                        next.range.end
                    }
                    _ => run.range.end,
                }
            }
        };
        (end <= self.trusted.end).then_some(end)
    }

    pub(crate) fn previous(&self, at: usize) -> Option<usize> {
        if at > self.trusted.end {
            return None;
        }
        let start = match self.preceding(at) {
            None => at,
            Some(index) => {
                let run = &self.runs[index];
                match (run.class, index.checked_sub(1)) {
                    (Class::Space, Some(before)) => self.runs[before].range.start,
                    _ => run.range.start,
                }
            }
        };
        (start >= self.trusted.start).then_some(start)
    }

    pub(crate) fn range_at(&self, at: usize, affinity: Affinity) -> Option<Range<usize>> {
        let found = match affinity {
            Affinity::After => self.containing(at).or_else(|| self.preceding(at)),
            Affinity::Before => self.preceding(at).or_else(|| self.containing(at)),
        };
        let range = match found {
            Some(index) => self.runs[index].range.clone(),
            None => at..at,
        };
        (range.start >= self.trusted.start && range.end <= self.trusted.end).then_some(range)
    }

    fn containing(&self, at: usize) -> Option<usize> {
        let index = self.runs.partition_point(|run| run.range.end <= at);
        self.runs
            .get(index)
            .filter(|run| run.range.start <= at)
            .map(|_| index)
    }

    fn preceding(&self, at: usize) -> Option<usize> {
        let index = self.runs.partition_point(|run| run.range.end < at);
        self.runs
            .get(index)
            .filter(|run| run.range.start < at)
            .map(|_| index)
    }
}

fn class_of(segment: &WordSegment<'_>) -> Class {
    match segment.text.chars().next() {
        Some(character) if character.is_whitespace() => Class::Space,
        _ if segment.is_word => Class::Word,
        _ => Class::Symbol,
    }
}

fn joins(left: Class, right: Class) -> bool {
    left == right && left != Class::Word
}
