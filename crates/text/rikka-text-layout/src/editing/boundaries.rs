use std::ops::Range;

use rikka_text_unicode::{WordRule, graphemes};

use crate::editing::cursor::Affinity;
use crate::editing::text_source::TextSource;
use crate::editing::window::{Window, find_in_window};
use crate::editing::word_runs::Runs;

pub fn next_grapheme_boundary(text: &(impl TextSource + ?Sized), index: usize) -> usize {
    find_in_window(text, index, |window, at| {
        graphemes(window.text())
            .map(|(start, cluster)| start + cluster.len())
            .find(|&end| end > at)
            .or_else(|| window.is_text_end().then_some(at))
            .map(|end| window.offset() + end)
    })
}

pub fn previous_grapheme_boundary(text: &(impl TextSource + ?Sized), index: usize) -> usize {
    find_in_window(text, index, |window, at| {
        graphemes(window.text())
            .map(|(start, _)| start)
            .take_while(|&start| start < at)
            .last()
            .or_else(|| window.is_text_start().then_some(0))
            .map(|start| window.offset() + start)
    })
}

pub fn word_range_at(
    text: &(impl TextSource + ?Sized),
    index: usize,
    affinity: Affinity,
    rule: WordRule,
) -> Range<usize> {
    find_in_window(text, index, |window, at| {
        Runs::new(window, rule)?
            .range_at(at, affinity)
            .map(|range| shifted(window, range))
    })
}

pub fn next_word_boundary(
    text: &(impl TextSource + ?Sized),
    index: usize,
    rule: WordRule,
) -> usize {
    find_in_window(text, index, |window, at| {
        Runs::new(window, rule)?
            .next(at)
            .map(|end| window.offset() + end)
    })
}

pub fn previous_word_boundary(
    text: &(impl TextSource + ?Sized),
    index: usize,
    rule: WordRule,
) -> usize {
    find_in_window(text, index, |window, at| {
        Runs::new(window, rule)?
            .previous(at)
            .map(|start| window.offset() + start)
    })
}

fn shifted(window: &Window<'_>, range: Range<usize>) -> Range<usize> {
    window.offset() + range.start..window.offset() + range.end
}
