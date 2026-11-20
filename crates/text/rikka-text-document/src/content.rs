use std::ops::Range;

use rikka_text_layout::{
    Affinity, Cursor, Movement, ParagraphLayout, WordRule, next_grapheme_boundary,
    next_word_boundary, previous_grapheme_boundary, previous_word_boundary, word_range_at,
};

use crate::change::{Changes, Splice, place_of};
use crate::granularity::Granularity;
use crate::history::{Edit, History};
use crate::line_ending::LineEnding;
use crate::rope::Rope;
use crate::travel::{Erase, Travel};

const DEPTH: usize = 512;

#[derive(Clone, Debug)]
struct Step {
    rope: Rope,
    caret: usize,
    anchor: usize,
}

#[derive(Debug)]
struct Ghost {
    text: String,
    revision: u64,
    caret: usize,
}

#[derive(Debug)]
pub struct Content {
    id: u64,
    rope: Rope,
    caret: usize,
    affinity: Affinity,
    anchor: usize,
    history: History<Step, DEPTH>,
    changes: Changes,
    line_ending: LineEnding,
    revision: u64,
    saved_at: u64,
    preedit: String,
    preedit_cursor: Option<(usize, usize)>,
    preedit_revision: u64,
    ghost: Option<Ghost>,
}

impl Default for Content {
    fn default() -> Self {
        Self::new("")
    }
}

impl Content {
    pub fn new(text: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let rope = Rope::from(&*LineEnding::normalize(text));
        let end = rope.len();
        Self {
            id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            rope,
            caret: end,
            affinity: Affinity::After,
            anchor: end,
            history: History::default(),
            changes: Changes::default(),
            line_ending: LineEnding::detect(text),
            revision: 0,
            saved_at: 0,
            preedit: String::new(),
            preedit_cursor: None,
            preedit_revision: 0,
            ghost: None,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    pub fn len(&self) -> usize {
        self.rope.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rope.is_empty()
    }

    pub fn char_count(&self) -> usize {
        self.rope.char_count()
    }

    pub fn line_count(&self) -> usize {
        self.rope.line_count()
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    pub fn affinity(&self) -> Affinity {
        self.affinity
    }

    pub fn anchor(&self) -> usize {
        self.anchor
    }

    pub fn selection(&self) -> Range<usize> {
        if self.anchor <= self.caret {
            self.anchor..self.caret
        } else {
            self.caret..self.anchor
        }
    }

    pub fn has_selection(&self) -> bool {
        self.anchor != self.caret
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn is_modified(&self) -> bool {
        self.revision != self.saved_at
    }

    pub fn mark_saved(&mut self) -> bool {
        let unsaved = self.saved_at != self.revision;
        self.saved_at = self.revision;
        unsaved
    }

    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    pub fn set_line_ending(&mut self, ending: LineEnding) -> bool {
        if self.line_ending == ending {
            return false;
        }
        self.line_ending = ending;
        self.revision += 1;
        true
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    pub fn selected_text(&self) -> String {
        self.slice(self.selection())
    }

    pub fn selected_char_count(&self) -> usize {
        let range = self.selection();
        self.rope.byte_to_char(range.end) - self.rope.byte_to_char(range.start)
    }
    pub fn slice(&self, range: Range<usize>) -> String {
        self.rope.slice(range)
    }

    pub fn preedit(&self) -> &str {
        &self.preedit
    }

    pub fn preedit_cursor(&self) -> Option<(usize, usize)> {
        self.preedit_cursor
    }

    pub fn set_preedit(&mut self, text: &str, cursor: Option<Range<usize>>) -> bool {
        let cursor = cursor.map(|cursor| {
            (
                text.floor_char_boundary(cursor.start),
                text.floor_char_boundary(cursor.end),
            )
        });
        if self.preedit == text && self.preedit_cursor == cursor {
            return false;
        }
        if self.preedit.is_empty() && !text.is_empty() {
            self.delete_selection();
        }
        self.preedit.clear();
        self.preedit.push_str(text);
        self.preedit_cursor = cursor;
        self.preedit_revision = self.preedit_revision.wrapping_add(1);
        true
    }

    pub fn clear_preedit(&mut self) -> bool {
        if self.preedit.is_empty() {
            return false;
        }
        self.preedit.clear();
        self.preedit_cursor = None;
        self.preedit_revision = self.preedit_revision.wrapping_add(1);
        true
    }

    pub fn preedit_revision(&self) -> u64 {
        self.preedit_revision
    }

    pub fn ghost(&self) -> &str {
        match &self.ghost {
            Some(ghost)
                if ghost.revision == self.revision
                    && ghost.caret == self.caret
                    && self.anchor == self.caret
                    && self.preedit.is_empty() =>
            {
                &ghost.text
            }
            _ => "",
        }
    }

    pub fn set_ghost(&mut self, text: &str) -> bool {
        if text.is_empty() {
            return self.clear_ghost();
        }
        if self.ghost() == text || self.has_selection() || !self.preedit.is_empty() {
            return false;
        }
        self.ghost = Some(Ghost {
            text: text.to_owned(),
            revision: self.revision,
            caret: self.caret,
        });
        self.preedit_revision = self.preedit_revision.wrapping_add(1);
        true
    }

    pub fn clear_ghost(&mut self) -> bool {
        let shown = !self.ghost().is_empty();
        self.ghost = None;
        if shown {
            self.preedit_revision = self.preedit_revision.wrapping_add(1);
        }
        shown
    }

    pub fn line_of(&self, at: usize) -> usize {
        self.rope.line_of(at)
    }

    pub fn line_start(&self, line: usize) -> usize {
        self.rope
            .line_start(line.min(self.line_count().saturating_sub(1)))
    }

    pub fn line_range(&self, line: usize) -> Range<usize> {
        self.rope.line_span(line)
    }

    pub fn line_into(&self, line: usize, out: &mut String) {
        out.extend(self.rope.chunks(self.line_range(line)));
    }

    pub fn caret_line_column(&self) -> (usize, usize) {
        let line = self.line_of(self.caret);
        let start = self.rope.line_start(line);
        let column = self.rope.byte_to_char(self.caret) - self.rope.byte_to_char(start);
        (line, column)
    }

    pub fn boundary(&self, at: usize) -> usize {
        self.rope.floor_char_boundary(at)
    }

    pub fn place_caret(&mut self, at: usize, extend: bool) -> bool {
        self.place_cursor(at, Affinity::After, extend)
    }

    pub fn place_cursor(&mut self, at: usize, affinity: Affinity, extend: bool) -> bool {
        let at = self.boundary(at);
        let moved =
            self.caret != at || self.affinity != affinity || (!extend && self.has_selection());
        self.caret = at;
        self.affinity = affinity;
        if !extend {
            self.anchor = at;
        }
        moved
    }

    pub fn select(&mut self, anchor: usize, caret: usize) -> bool {
        let (anchor, caret) = (self.boundary(anchor), self.boundary(caret));
        let moved = self.anchor != anchor || self.caret != caret;
        self.anchor = anchor;
        self.caret = caret;
        self.affinity = facing(anchor, caret);
        moved
    }

    pub fn select_all(&mut self) -> bool {
        if self.is_empty() || (self.anchor == 0 && self.caret == self.len()) {
            return false;
        }
        self.select(0, self.len())
    }

    pub fn unit_at(
        &self,
        at: usize,
        affinity: Affinity,
        granularity: Granularity,
        rule: WordRule,
    ) -> Range<usize> {
        let at = self.boundary(at);
        match granularity {
            Granularity::Character => at..at,
            Granularity::Word => word_range_at(&self.rope, at, affinity, rule),
            Granularity::Line => {
                let line = self.line_of(at);
                self.rope.line_start(line)..self.rope.line_start(line + 1)
            }
        }
    }

    pub fn replace(&mut self, range: Range<usize>, text: &str) -> bool {
        self.put(range, text, Edit::Other)
    }

    pub fn replace_each(&mut self, edits: &[(Range<usize>, String)]) -> bool {
        if edits.is_empty() {
            return false;
        }
        self.history.record(&self.step(), Edit::Other);
        let mut caret = self.caret;
        for (range, text) in edits.iter().rev() {
            let text = LineEnding::normalize(text);
            let start = self.boundary(range.start);
            let end = self.boundary(range.end).max(start);
            if start == end && text.is_empty() {
                continue;
            }
            self.revision += 1;
            self.swap(start..end, &text);
            caret = start + text.len();
        }
        self.collapse(caret);
        true
    }

    fn put(&mut self, range: Range<usize>, text: &str, kind: Edit) -> bool {
        let text = LineEnding::normalize(text);
        let start = self.boundary(range.start);
        let end = self.boundary(range.end).max(start);
        if start == end && text.is_empty() {
            return false;
        }
        self.history.record(&self.step(), kind);
        self.revision += 1;
        self.swap(start..end, &text);
        self.collapse(start + text.len());
        if !text.is_empty() {
            self.affinity = Affinity::Before;
        }
        true
    }

    fn swap(&mut self, range: Range<usize>, text: &str) {
        self.splice(range.start, range.end, range.start + text.len(), |rope| {
            rope.remove(range.clone());
            rope.insert(range.start, text);
        });
    }

    fn splice(
        &mut self,
        start: usize,
        old_end: usize,
        new_end: usize,
        edit: impl FnOnce(&mut Rope),
    ) {
        let start_place = place_of(&self.rope, start);
        let old_end_place = place_of(&self.rope, old_end);
        edit(&mut self.rope);
        self.changes.record(
            self.revision,
            Splice {
                start,
                old_end,
                new_end,
                start_place,
                old_end_place,
                new_end_place: place_of(&self.rope, new_end),
            },
        );
    }

    pub fn changes_since(&self, revision: u64) -> Option<impl Iterator<Item = Splice> + '_> {
        self.changes.since(revision, self.revision)
    }

    pub fn insert_str(&mut self, text: &str) -> bool {
        let mut typed = text.chars();
        let kind = match (typed.next(), typed.next()) {
            (Some(ch), None) if ch != '\n' => Edit::Typing,
            _ => Edit::Other,
        };
        self.put(self.selection(), text, kind)
    }

    pub fn insert_newline(&mut self, auto_indent: bool) -> bool {
        let mut text = String::from("\n");
        if auto_indent {
            let start = self.selection().start;
            let line = self.line_range(self.line_of(start));
            let indent = self.rope.chunks(line.start..line.end.min(start));
            text.extend(
                indent
                    .flat_map(str::chars)
                    .take_while(|ch| *ch == ' ' || *ch == '\t'),
            );
        }
        self.put(self.selection(), &text, Edit::Other)
    }

    pub fn backspace(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        self.put(self.previous()..self.caret, "", Edit::Deleting)
    }

    pub fn delete(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        let next = next_grapheme_boundary(&self.rope, self.caret);
        self.put(self.caret..next, "", Edit::Deleting)
    }

    fn delete_word_back(&mut self, rule: WordRule) -> bool {
        if self.delete_selection() {
            return true;
        }
        let start = previous_word_boundary(&self.rope, self.caret, rule);
        self.put(start..self.caret, "", Edit::Deleting)
    }

    fn delete_word_forward(&mut self, rule: WordRule) -> bool {
        if self.delete_selection() {
            return true;
        }
        let end = next_word_boundary(&self.rope, self.caret, rule);
        self.put(self.caret..end, "", Edit::Deleting)
    }

    pub fn erase(&mut self, erase: Erase, rule: WordRule) -> bool {
        match erase {
            Erase::Back => self.backspace(),
            Erase::Forward => self.delete(),
            Erase::WordBack => self.delete_word_back(rule),
            Erase::WordForward => self.delete_word_forward(rule),
        }
    }

    pub fn delete_selection(&mut self) -> bool {
        self.has_selection() && self.put(self.selection(), "", Edit::Deleting)
    }

    pub fn indent(&mut self, unit: &str) -> bool {
        self.history.record(&self.step(), Edit::Other);
        self.revision += 1;
        for line in self.touched_lines().rev() {
            let at = self.rope.line_start(line);
            self.splice(at, at, at + unit.len(), |rope| rope.insert(at, unit));
            self.shift_after(at, unit.len() as isize);
        }
        true
    }

    pub fn outdent(&mut self, unit: &str) -> bool {
        let mut removed_any = false;
        for line in self.touched_lines().rev() {
            let range = self.line_range(line);
            let removed = self.outdent_width(range.clone(), unit);
            if removed == 0 {
                continue;
            }
            if !removed_any {
                self.history.record(&self.step(), Edit::Other);
                self.revision += 1;
                removed_any = true;
            }
            let start = range.start;
            self.splice(start, start + removed, start, |rope| {
                rope.remove(start..start + removed);
            });
            self.shift_after(start, -(removed as isize));
        }
        removed_any
    }

    pub fn set_text(&mut self, text: &str) -> bool {
        let normalized = LineEnding::normalize(text);
        if self.rope == *normalized {
            let cleared = self.clear_preedit();
            let end = self.len();
            let moved = self.caret != end || self.anchor != end;
            self.collapse(end);
            return cleared || moved;
        }
        self.clear_preedit();
        self.history.record(&self.step(), Edit::Other);
        self.revision += 1;
        let replacement = Rope::from(&*normalized);
        let (start, old_end, new_end) = spread(&self.rope, &replacement);
        self.splice(start, old_end, new_end, |rope| *rope = replacement);
        self.collapse(self.len());
        true
    }

    pub fn append(&mut self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        let text = LineEnding::normalize(text);
        let start = self.len();
        self.history.record(&self.step(), Edit::Appending);
        self.revision += 1;
        self.splice(start, start, start + text.len(), |rope| {
            rope.insert(start, &text);
        });
        true
    }

    pub fn reset(&mut self, text: &str) {
        self.rope = Rope::from(&*LineEnding::normalize(text));
        self.line_ending = LineEnding::detect(text);
        self.history.clear();
        self.changes.forget();
        self.collapse(0);
        self.revision += 1;
        self.saved_at = self.revision;
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn undo(&mut self) -> bool {
        let Some(step) = self.history.undo(&self.step()) else {
            return false;
        };
        self.restore(step);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(step) = self.history.redo(&self.step()) else {
            return false;
        };
        self.restore(step);
        true
    }

    pub fn interrupt(&mut self) {
        self.history.interrupt();
    }

    pub fn travel(
        &mut self,
        to: Travel,
        extend: bool,
        line: Option<&ParagraphLayout>,
        rule: WordRule,
    ) -> bool {
        let rtl = line.is_some_and(|line| line.paragraph().is_rtl());
        let (at, affinity) = match to {
            Travel::Left | Travel::Right if !extend && self.has_selection() => {
                let range = self.selection();
                match (to == Travel::Right) != rtl {
                    true => (range.end, Affinity::Before),
                    false => (range.start, Affinity::After),
                }
            }
            Travel::Left => self.visual_step(Movement::Left, line),
            Travel::Right => self.visual_step(Movement::Right, line),
            Travel::WordLeft | Travel::WordRight => {
                let at = match (to == Travel::WordRight) != rtl {
                    true => next_word_boundary(&self.rope, self.caret, rule),
                    false => previous_word_boundary(&self.rope, self.caret, rule),
                };
                (at, Affinity::After)
            }
            Travel::LineStart => self.row_start(line),
            Travel::LineEnd => self.row_end(line),
            Travel::Start => (0, Affinity::After),
            Travel::End => (self.len(), Affinity::Before),
        };
        self.place_cursor(at, affinity, extend)
    }

    fn visual_step(&self, movement: Movement, line: Option<&ParagraphLayout>) -> (usize, Affinity) {
        let Some(laid) = line else {
            let at = match movement {
                Movement::Left => previous_grapheme_boundary(&self.rope, self.caret),
                Movement::Right => next_grapheme_boundary(&self.rope, self.caret),
            };
            return (at, Affinity::After);
        };
        let line = self.line_of(self.caret);
        let start = self.rope.line_start(line);
        if let Some(cursor) = laid.visual_step(self.cursor_in(start), movement) {
            return (start + cursor.index, cursor.affinity);
        }
        let forward = (movement == Movement::Right) != laid.paragraph().is_rtl();
        match forward {
            true if line + 1 < self.line_count() => {
                (self.rope.line_start(line + 1), Affinity::After)
            }
            false if line > 0 => (self.line_range(line - 1).end, Affinity::Before),
            _ => (self.caret, self.affinity),
        }
    }

    fn row_start(&self, line: Option<&ParagraphLayout>) -> (usize, Affinity) {
        let range = self.line_range(self.line_of(self.caret));
        let row = line.map_or(0, |laid| laid.row_start(self.cursor_in(range.start)).index);
        if row > 0 {
            return (range.start + row, Affinity::After);
        }
        let indented = self
            .rope
            .chunks(range.clone())
            .flat_map(str::chars)
            .take_while(|ch| *ch == ' ' || *ch == '\t')
            .map(char::len_utf8)
            .sum::<usize>();
        let text_starts = range.start + indented;
        match self.caret == text_starts {
            true => (range.start, Affinity::After),
            false => (text_starts, Affinity::After),
        }
    }

    fn row_end(&self, line: Option<&ParagraphLayout>) -> (usize, Affinity) {
        let range = self.line_range(self.line_of(self.caret));
        let end = line.map_or(range.len(), |laid| {
            laid.row_end(self.cursor_in(range.start)).index
        });
        (range.start + end.min(range.len()), Affinity::Before)
    }

    fn cursor_in(&self, line_start: usize) -> Cursor {
        Cursor::new(self.caret - line_start, self.affinity)
    }

    fn step(&self) -> Step {
        Step {
            rope: self.rope.clone(),
            caret: self.caret,
            anchor: self.anchor,
        }
    }

    fn restore(&mut self, step: Step) {
        self.revision += 1;
        let (start, old_end, new_end) = spread(&self.rope, &step.rope);
        self.splice(start, old_end, new_end, |rope| *rope = step.rope);
        self.caret = step.caret.min(self.len());
        self.anchor = step.anchor.min(self.len());
        self.affinity = Affinity::After;
    }

    fn collapse(&mut self, at: usize) {
        let at = self.boundary(at);
        self.caret = at;
        self.anchor = at;
        self.affinity = Affinity::After;
    }

    fn touched_lines(&self) -> Range<usize> {
        let range = self.selection();
        let first = self.line_of(range.start);
        let mut last = self.line_of(range.end);
        if last > first && range.end == self.rope.line_start(last) {
            last -= 1;
        }
        first..last + 1
    }

    fn outdent_width(&self, line: Range<usize>, unit: &str) -> usize {
        let blanks: Vec<char> = self
            .rope
            .chunks(line)
            .flat_map(str::chars)
            .take(unit.chars().count())
            .take_while(|ch| *ch == ' ' || *ch == '\t')
            .collect();
        if blanks.iter().copied().eq(unit.chars()) {
            return unit.len();
        }
        blanks.iter().copied().map(char::len_utf8).sum()
    }

    fn shift_after(&mut self, at: usize, by: isize) {
        let len = self.rope.len();
        let shift = |offset: usize| match offset > at {
            true => offset.saturating_add_signed(by).clamp(at, len),
            false => offset,
        };
        self.caret = shift(self.caret);
        self.anchor = shift(self.anchor);
    }

    fn previous(&self) -> usize {
        self.rope
            .char_before(self.caret)
            .map_or(self.caret, |ch| self.caret - ch.len_utf8())
    }
}

fn facing(anchor: usize, caret: usize) -> Affinity {
    match caret > anchor {
        true => Affinity::Before,
        false => Affinity::After,
    }
}

fn spread(before: &Rope, after: &Rope) -> (usize, usize, usize) {
    let head: usize = before
        .chars_from(0)
        .zip(after.chars_from(0))
        .take_while(|(one, other)| one == other)
        .map(|(one, _)| one.len_utf8())
        .sum();
    let room = before.len().min(after.len()) - head;
    let mut tail = 0;
    for (one, other) in before
        .chars_before(before.len())
        .zip(after.chars_before(after.len()))
    {
        if one != other || tail + one.len_utf8() > room {
            break;
        }
        tail += one.len_utf8();
    }
    (head, before.len() - tail, after.len() - tail)
}
