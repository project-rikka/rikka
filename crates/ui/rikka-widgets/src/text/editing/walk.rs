use std::sync::Arc;

use rikka_text::{Affinity, Cursor};
use rikka_ui::text::{DisplayText, TextLayout, TextRun};

use crate::text::editing::rows::Rows;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Step {
    pub(crate) rows: i32,
    pub(crate) extend: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Target {
    pub(crate) at: usize,
    pub(crate) affinity: Affinity,
    pub(crate) aim: f32,
}

pub(crate) fn take(
    shown: &DisplayText<'_>,
    rows: &Rows,
    template: &TextRun,
    width: Option<f32>,
    step: Step,
    goal: Option<f32>,
) -> Target {
    let caret = shown.caret();
    let line = shown.line_of(caret);
    let (row, x) = at(shown, &line_layout(shown, template, width, line), line);
    let aim = goal.unwrap_or(x);

    let last = rows.total().saturating_sub(1);
    let from = i64::from(rows.before(line) + row);
    let to = (from + i64::from(step.rows)).clamp(0, i64::from(last)) as u32;
    let (target_line, target_row) = rows.at(to);

    let laid = line_layout(shown, template, width, target_line);
    let middle = laid.line_height() * (target_row as f32 + 0.5);
    let hit = laid.hit(aim, middle);
    Target {
        at: shown.document_offset(shown.line_start(target_line) + laid.offset_of(hit)),
        affinity: hit.affinity,
        aim,
    }
}

pub(crate) fn place_caret(
    shown: &DisplayText<'_>,
    template: &TextRun,
    width: Option<f32>,
) -> (u32, f32, f32) {
    let line = shown.line_of(shown.caret());
    let laid = line_layout(shown, template, width, line);
    let (row, x) = at(shown, &laid, line);
    (row, x, laid.widest())
}

pub(crate) fn line_layout(
    shown: &DisplayText<'_>,
    template: &TextRun,
    width: Option<f32>,
    line: usize,
) -> Arc<TextLayout> {
    let mut text = String::new();
    shown.line_into(line, &mut text);
    let run = TextRun {
        content: text.into(),
        ..template.clone()
    };
    rikka_text::layout(run.as_shape(), width)
}

fn at(shown: &DisplayText<'_>, laid: &TextLayout, line: usize) -> (u32, f32) {
    let within = shown.caret() - shown.line_start(line);
    let caret = laid.caret(Cursor {
        line: 0,
        index: within,
        affinity: shown.affinity(),
    });
    (caret.row as u32, caret.x)
}
