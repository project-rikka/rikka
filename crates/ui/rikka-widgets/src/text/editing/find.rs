use std::ops::Range;

use rikka_types::Point;
use rikka_ui::search::{Aim, Marking, Query, Scan, Step, Tally};
use rikka_ui::text::{DisplayRevision, DisplayText};
use rikka_ui::{PaintCtx, Quad};

use crate::text::caret;
use crate::text::editing::Editing;
use crate::text::editing::block::Piece;
use crate::text::editing::state::EditingState;

const REACH: usize = 8 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
struct MarkedRange {
    query: u64,
    revision: DisplayRevision,
    window: Range<usize>,
}

#[derive(Debug, Default)]
pub(crate) struct FindState {
    scan: Scan,
    marked: Option<MarkedRange>,
    marks: Vec<Range<usize>>,
    current: Option<Range<usize>>,
}

impl FindState {
    fn mark(&mut self, shown: &DisplayText<'_>, query: &Query, window: Range<usize>) {
        let content = shown.content();
        let marked = MarkedRange {
            query: query.stamp(),
            revision: shown.revision(),
            window: window.clone(),
        };
        if self.marked.as_ref() == Some(&marked) {
            return;
        }
        self.marked = Some(marked);

        let from = window.start.saturating_sub(REACH);
        let to = (window.end + REACH).min(content.len());
        let read = content.slice(from..to);
        self.marks.clear();
        self.marks.extend(
            query
                .matches(&read)
                .map(|range| from + range.start..from + range.end)
                .filter(|range| range.end > window.start && range.start < window.end),
        );
    }

    fn forget_marks(&mut self) {
        self.marked = None;
        self.marks.clear();
    }

    fn forget(&mut self) {
        self.scan.reset();
        self.forget_marks();
        self.current = None;
    }
}

impl Editing {
    pub(crate) fn find(&self, state: &mut EditingState, ctx: &mut PaintCtx<'_>) {
        let Some(search) = &self.search else {
            state.finding.forget();
            return;
        };
        let query = search.query();
        if !search.is_open() || !query.is_readable() {
            state.finding.forget();
            if search.report(Tally::default()) {
                ctx.request_view();
            }
            return;
        }

        match search.take_step() {
            Some(Step::Replace) => self.replace_one(state, &query, &search.replacement()),
            Some(Step::ReplaceAll) => self.replace_all(state, &query, &search.replacement()),
            Some(step) => self.look(state, &query, Aim::from_step(step)),
            None => self.look(state, &query, Aim::Nearest),
        }
        let result = state.finding.scan.result();
        let tally = Tally::new(result.count(), result.current());
        state.finding.current = result.current_range();
        if let Some(target) = state.finding.scan.take_navigation_target() {
            self.select_match(state, target);
        }

        let pending = state.finding.scan.is_pending();
        let changed = match pending {
            true => search.reporting(tally),
            false => search.report(tally),
        };
        if changed {
            ctx.request_view();
        }
    }

    fn look(&self, state: &mut EditingState, query: &Query, aim: Aim) {
        let content = self.document.content();
        state.finding.scan.request(
            query,
            content.rope(),
            content.revision(),
            content.selection(),
            aim,
        );
    }

    fn select_match(&self, state: &mut EditingState, range: Range<usize>) {
        self.document.select(range.start, range.end);
        state.goal = None;
        state.follow = true;
    }

    fn replace_one(&self, state: &mut EditingState, query: &Query, replacement_text: &str) {
        if !self.editable() {
            return;
        }
        let Some(range) = state.finding.current.clone() else {
            return;
        };
        let matched = self.document.content().slice(range.clone());
        let replacement_text = query.expand_replacement(&matched, replacement_text);
        self.document.replace(range, &replacement_text);
        state.follow = true;
        self.look(state, query, Aim::Forward);
    }

    fn replace_all(&self, state: &mut EditingState, query: &Query, replacement_text: &str) {
        if !self.editable() {
            return;
        }
        let text = self.document.text();
        let edits: Vec<(Range<usize>, String)> = query
            .matches(&text)
            .filter_map(|range| {
                let matched = text.get(range.clone())?;
                Some((range, query.expand_replacement(matched, replacement_text)))
            })
            .collect();
        if !self.document.replace_each(&edits) {
            return;
        }
        state.follow = true;
        self.look(state, query, Aim::Nearest);
    }

    pub(crate) fn found(
        &self,
        state: &mut EditingState,
        ctx: &PaintCtx<'_>,
        shown: &DisplayText<'_>,
        drawn: Range<usize>,
    ) -> Option<Marking> {
        let Some(query) = self
            .search
            .as_ref()
            .filter(|search| search.is_open())
            .map(|search| search.query())
            .filter(Query::is_readable)
        else {
            state.finding.forget_marks();
            return None;
        };
        let window = shown.document_offset(drawn.start)..shown.document_offset(drawn.end);
        state.finding.mark(shown, &query, window);
        Some(Marking::from_palette(ctx.theme().palette()))
    }
}

impl FindState {
    pub(crate) fn draw(
        &self,
        ctx: &mut PaintCtx<'_>,
        shown: &DisplayText<'_>,
        piece: &Piece,
        marking: &Marking,
        top: Point,
    ) {
        let drawn = piece.drawn(shown);
        for found in &self.marks {
            let range = shown.range(found.clone());
            let start = range.start.max(drawn.start);
            let end = range.end.min(drawn.end);
            if start >= end {
                continue;
            }
            let ink = marking.color(self.current.as_ref() == Some(found));
            for band in piece.selection(shown, start..end) {
                ctx.draw_quad(
                    Quad::new(band.translate(top))
                        .fill(ink)
                        .radius(caret::SELECTION_RADIUS),
                );
            }
        }
    }
}
