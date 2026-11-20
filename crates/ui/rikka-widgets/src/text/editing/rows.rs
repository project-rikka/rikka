use rikka_sync::Mutex;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rikka_ui::Family;
use rikka_ui::in_background;
use rikka_ui::text::{BULLET, DisplayText, TextRun};
use rustc_hash::FxHashMap;

use crate::sums::Sums;

const WIDEST_ADVANCE_PER_BYTE: f32 = 1.0;

const KEPT_LINES: usize = 4096;

const MARGIN: usize = 64;

const WORTH_GUESSING: usize = 512;

#[derive(Clone, Debug, PartialEq)]
struct Shaping {
    width: f32,
    size: f32,
    line_height: f32,
    tab_width: u16,
    family: Family,
    bullets: bool,
}

impl Shaping {
    fn widened(&self, other: &Shaping) -> bool {
        self.size == other.size
            && self.line_height == other.line_height
            && self.tab_width == other.tab_width
            && self.family == other.family
            && self.bullets == other.bullets
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Read {
    revision: u64,
    composing: Option<usize>,
    composed: u64,
}

#[derive(Debug, Default)]
struct Handoff {
    abandoned: AtomicBool,
    counted: Mutex<Option<Vec<(usize, u32)>>>,
}

impl Handoff {
    fn abandoned(&self) -> bool {
        self.abandoned.load(Ordering::Relaxed)
    }

    fn hand_over(&self, rows: Vec<(usize, u32)>) {
        *self.counted.lock() = Some(rows);
    }

    fn take(&self) -> Option<Vec<(usize, u32)>> {
        self.counted.lock().take()
    }
}

#[derive(Debug)]
struct Pending {
    read: Read,
    width: u32,
    handoff: Arc<Handoff>,
}

impl Drop for Pending {
    fn drop(&mut self) {
        self.handoff.abandoned.store(true, Ordering::Relaxed);
    }
}

#[derive(Debug, Default)]
pub(crate) struct Rows {
    shaping: Option<Shaping>,
    read: Option<Read>,
    last_read: Option<(u64, u64)>,
    index: Index,
    lines: usize,
    measured: FxHashMap<Box<str>, u32>,
    pending: Option<Pending>,
}

impl Rows {
    pub(crate) fn sync(
        &mut self,
        shown: &DisplayText<'_>,
        template: &TextRun,
        width: Option<f32>,
        near: Range<usize>,
    ) -> i64 {
        let content = shown.content();
        let read = (content.id(), content.revision());
        let anchor = near.start;
        let was = (self.last_read.replace(read) == Some(read)).then(|| self.before(anchor));
        self.count(shown, template, width, near);
        was.map_or(0, |was| i64::from(self.before(anchor)) - i64::from(was))
    }

    fn count(
        &mut self,
        shown: &DisplayText<'_>,
        template: &TextRun,
        width: Option<f32>,
        near: Range<usize>,
    ) {
        self.lines = shown.line_count();
        let Some(width) = width.filter(|width| *width > 0.0) else {
            self.forget();
            return;
        };
        let shaping = Shaping {
            width,
            size: template.size,
            line_height: template.line_height,
            tab_width: template.tab_width,
            family: template.family.clone(),
            bullets: shown.is_secret(),
        };
        let read = Read {
            revision: shown.content().revision(),
            composing: shown.composing_line(),
            composed: shown.content().preedit_revision(),
        };
        let mut counting = RowMeasurer {
            template,
            width,
            certain: width / (template.size * WIDEST_ADVANCE_PER_BYTE),
            measured: std::mem::take(&mut self.measured),
            line: String::new(),
        };
        match &self.shaping {
            Some(cached) if *cached == shaping => {
                self.take_counted(read, &shaping);
                if self.read != Some(read) && !self.caught_up(shown, read, &mut counting) {
                    self.count_all(shown, read, &mut counting);
                }
            }
            Some(cached) if cached.widened(&shaping) && self.guesses(read) => {
                let was = cached.width;
                counting.measured.clear();
                self.pending = None;
                self.guess(shown, read, &mut counting, near, was);
            }
            _ => {
                self.forget();
                counting.measured.clear();
                self.count_all(shown, read, &mut counting);
            }
        }
        if counting.measured.len() > KEPT_LINES {
            counting.measured.clear();
        }
        self.measured = counting.measured;
        self.shaping = Some(shaping);
        self.read = Some(read);
        if self.pending.is_none() && self.index.guessing() {
            self.count_behind(shown, template, width, read);
        }
    }

    pub(crate) fn settle(
        &mut self,
        shown: &DisplayText<'_>,
        template: &TextRun,
        lines: Range<usize>,
    ) -> bool {
        let (Some(shaping), Some(read)) = (&self.shaping, self.read) else {
            return false;
        };
        let guessed: Vec<usize> = lines.filter(|line| self.index.guessed(*line)).collect();
        if guessed.is_empty() {
            return false;
        }
        let mut counting = RowMeasurer {
            template,
            width: shaping.width,
            certain: shaping.width / (template.size * WIDEST_ADVANCE_PER_BYTE),
            measured: std::mem::take(&mut self.measured),
            line: String::new(),
        };
        for line in guessed {
            let rows = counting.rows(shown, line, read.composing);
            self.index.settle(line, rows);
        }
        self.measured = counting.measured;
        true
    }

    pub(crate) fn counting(&self) -> bool {
        self.pending.is_some()
    }

    fn guesses(&self, read: Read) -> bool {
        self.read == Some(read) && self.lines >= WORTH_GUESSING && self.index.len() == self.lines
    }

    fn guess(
        &mut self,
        shown: &DisplayText<'_>,
        read: Read,
        counting: &mut RowMeasurer<'_>,
        near: Range<usize>,
        was: f32,
    ) {
        let near = near.start.saturating_sub(MARGIN)..(near.end + MARGIN).min(self.lines);
        let stretch = was / counting.width;
        let mut heights = Vec::with_capacity(self.lines);
        let mut guessed = Vec::with_capacity(self.lines);
        for line in 0..self.lines {
            if near.contains(&line) || read.composing == Some(line) {
                heights.push(counting.rows(shown, line, read.composing));
                guessed.push(false);
                continue;
            }
            let cached = self.index.height(line);
            let rows = match cached {
                1 if stretch <= 1.0 => 1,
                _ => ((cached as f32 * stretch).round() as u32).max(1),
            };
            heights.push(rows);
            guessed.push(cached > 1 || stretch > 1.0);
        }
        self.index.fill_guessed(heights, guessed);
    }

    fn count_behind(
        &mut self,
        shown: &DisplayText<'_>,
        template: &TextRun,
        width: f32,
        read: Read,
    ) {
        let lines: Vec<usize> = (0..self.lines)
            .filter(|line| self.index.guessed(*line))
            .collect();
        let rope = shown.content().rope().clone();
        let bullets = shown.is_secret();
        let handoff = Arc::new(Handoff::default());
        let job = Arc::clone(&handoff);
        let template = template.clone();
        in_background(move || {
            let mut text = String::new();
            let mut counted = Vec::with_capacity(lines.len());
            for line in lines {
                if job.abandoned() {
                    return;
                }
                let drawn = rope.line_span(line);
                text.clear();
                match bullets {
                    true => {
                        let letters = rope.byte_to_char(drawn.end) - rope.byte_to_char(drawn.start);
                        text.extend(std::iter::repeat_n(BULLET, letters));
                    }
                    false => text.extend(rope.chunks(drawn)),
                }
                counted.push((line, rows_of(&template, width, &text)));
            }
            job.hand_over(counted);
        });
        self.pending = Some(Pending {
            read,
            width: width.to_bits(),
            handoff,
        });
    }

    fn take_counted(&mut self, read: Read, shaping: &Shaping) {
        let Some(pending) = &self.pending else {
            return;
        };
        if pending.read != read || pending.width != shaping.width.to_bits() {
            self.pending = None;
            return;
        }
        let Some(counted) = pending.handoff.take() else {
            return;
        };
        self.pending = None;
        for (line, rows) in counted {
            if self.index.guessed(line) {
                self.index.settle(line, rows);
            }
        }
    }

    fn caught_up(
        &mut self,
        shown: &DisplayText<'_>,
        read: Read,
        counting: &mut RowMeasurer<'_>,
    ) -> bool {
        let Some(cached) = self.read else {
            return false;
        };
        if self.index.is_empty() {
            return false;
        }
        let Some(mut splices) = shown.content().changes_since(cached.revision) else {
            return false;
        };
        let edit = splices.next();
        if splices.next().is_some() {
            return false;
        }

        if let Some(splice) = edit {
            let first = splice.start_place.row;
            let before = splice.old_end_place.row + 1 - first;
            let now = splice.new_end_place.row + 1 - first;
            let counted: Vec<u32> = (first..first + now)
                .map(|line| counting.rows(shown, line, read.composing))
                .collect();
            self.index.splice(first, before, &counted);
        }

        for line in [cached.composing, read.composing].into_iter().flatten() {
            if line < self.lines {
                let rows = counting.rows(shown, line, read.composing);
                self.index.settle(line, rows);
            }
        }
        self.index.len() == self.lines
    }

    fn count_all(&mut self, shown: &DisplayText<'_>, read: Read, counting: &mut RowMeasurer<'_>) {
        let mut heights = Vec::with_capacity(self.lines);
        for line in 0..self.lines {
            heights.push(counting.rows(shown, line, read.composing));
        }
        self.pending = None;
        self.index.fill(heights);
    }

    fn forget(&mut self) {
        self.shaping = None;
        self.read = None;
        self.measured = FxHashMap::default();
        self.index = Index::default();
        self.pending = None;
    }

    fn wraps(&self) -> bool {
        !self.index.is_empty()
    }

    pub(crate) fn before(&self, line: usize) -> u32 {
        match self.wraps() {
            true => self.index.before(line.min(self.lines)),
            false => line.min(self.lines) as u32,
        }
    }

    pub(crate) fn total(&self) -> u32 {
        self.before(self.lines)
    }

    pub(crate) fn row_count(&self, line: usize) -> u32 {
        self.before(line + 1)
            .saturating_sub(self.before(line))
            .max(1)
    }

    pub(crate) fn at(&self, row: u32) -> (usize, u32) {
        if !self.wraps() {
            let line = (row as usize).min(self.lines.saturating_sub(1));
            return (line, 0);
        }
        self.index.at(row, self.lines)
    }
}

#[derive(Debug, Default)]
struct Index {
    heights: Vec<u32>,
    sums: Sums<u32>,
    guessed: Vec<bool>,
    guesses: usize,
}

impl Index {
    fn is_empty(&self) -> bool {
        self.heights.is_empty()
    }

    fn len(&self) -> usize {
        self.heights.len()
    }

    fn height(&self, line: usize) -> u32 {
        self.heights.get(line).copied().unwrap_or(1)
    }

    fn guessing(&self) -> bool {
        self.guesses > 0
    }

    fn guessed(&self, line: usize) -> bool {
        self.guessed.get(line).copied().unwrap_or(false)
    }

    fn fill(&mut self, heights: Vec<u32>) {
        self.guessed = vec![false; heights.len()];
        self.guesses = 0;
        self.heights = heights;
        self.gather();
    }

    fn fill_guessed(&mut self, heights: Vec<u32>, guessed: Vec<bool>) {
        self.guesses = guessed.iter().filter(|guess| **guess).count();
        self.guessed = guessed;
        self.heights = heights;
        self.gather();
    }

    fn splice(&mut self, at: usize, removed: usize, counted: &[u32]) {
        let at = at.min(self.heights.len());
        let end = (at + removed).min(self.heights.len());
        if removed == counted.len() {
            for (line, rows) in counted.iter().enumerate() {
                self.settle(at + line, *rows);
            }
            return;
        }
        let removed_guesses = self.guessed[at..end].iter().filter(|guess| **guess).count();
        self.guesses -= removed_guesses;
        self.guessed
            .splice(at..end, std::iter::repeat_n(false, counted.len()));
        self.heights.splice(at..end, counted.iter().copied());
        self.gather();
    }

    fn settle(&mut self, line: usize, rows: u32) {
        if let Some(guess) = self.guessed.get_mut(line)
            && std::mem::replace(guess, false)
        {
            self.guesses -= 1;
        }
        self.set(line, rows);
    }

    fn set(&mut self, line: usize, rows: u32) {
        let Some(cached) = self.heights.get_mut(line) else {
            return;
        };
        if *cached == rows {
            return;
        }
        let was = std::mem::replace(cached, rows);
        match rows >= was {
            true => self.sums.add(line, rows - was),
            false => self.sums.subtract(line, was - rows),
        }
    }

    fn before(&self, line: usize) -> u32 {
        self.sums.before(line.min(self.heights.len()))
    }

    fn at(&self, row: u32, lines: usize) -> (usize, u32) {
        let mut left = row;
        let line = self.sums.find(|rows| {
            let fits = rows <= left;
            if fits {
                left -= rows;
            }
            fits
        });
        (line.min(lines.saturating_sub(1)), left)
    }

    fn gather(&mut self) {
        self.sums = Sums::from_values(self.heights.iter().copied());
    }
}

struct RowMeasurer<'a> {
    template: &'a TextRun,
    width: f32,
    certain: f32,
    measured: FxHashMap<Box<str>, u32>,
    line: String,
}

impl RowMeasurer<'_> {
    fn rows(&mut self, shown: &DisplayText<'_>, line: usize, composing: Option<usize>) -> u32 {
        let composed = composing == Some(line);
        let drawn = shown.line_range(line);
        let short = ((drawn.end - drawn.start) as f32) < self.certain && !composed;
        if short && !tabbed(shown, line) {
            return 1;
        }
        self.line.clear();
        shown.line_into(line, &mut self.line);
        let text = std::mem::take(&mut self.line);
        let rows = match composed {
            true => rows_of(self.template, self.width, &text),
            false => self.remembered(&text),
        };
        self.line = text;
        rows
    }

    fn remembered(&mut self, line: &str) -> u32 {
        if let Some(rows) = self.measured.get(line) {
            return *rows;
        }
        let rows = rows_of(self.template, self.width, line);
        self.measured.insert(line.into(), rows);
        rows
    }
}

fn rows_of(template: &TextRun, width: f32, line: &str) -> u32 {
    if line.is_empty() {
        return 1;
    }
    let run = TextRun {
        content: line.into(),
        ..template.clone()
    };
    rikka_text::layout(run.as_shape(), Some(width))
        .row_count()
        .max(1) as u32
}

fn tabbed(shown: &DisplayText<'_>, line: usize) -> bool {
    let content = shown.content();
    content
        .rope()
        .chunks(content.line_range(line))
        .any(|chunk| chunk.contains('\t'))
}
