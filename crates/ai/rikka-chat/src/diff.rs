use std::sync::Arc;

use rikka_file_edit::{Change, Line};
use rikka_syntax::Language;

use crate::card::{CARD_INSET, card};
use crate::chat::Labels;
use crate::code::{self, BESIDE, Ink, Rows, TALLEST};
use crate::mono::{CODE_FONT_SIZE, mono};
use crate::prelude::*;
use rikka_widgets::{column, row};

const GUTTER: f32 = 16.0;

pub(crate) fn changed<C: Labels>(change: &Change) -> Container {
    let mut rows = Rows::upto(BESIDE);
    let language = Language::from_path(&change.path);

    for (index, hunk) in change.hunks.iter().enumerate() {
        if index > 0 {
            rows.push(rule);
        }
        let at = hunk.start_line;
        rows.push(move || line_number(at));
        let inked: Vec<(Ink, &str)> = hunk.lines.iter().map(inked).collect();
        let painted = code::highlighted_code(language, theme().syntax(), &inked);
        for (line, spans) in hunk.lines.iter().zip(painted) {
            let line = line.clone();
            rows.push(move || line_row(&line, spans.clone()));
        }
    }

    file::<C>(
        change.path.clone(),
        code::diff_stat(change.added, change.removed),
        rows,
    )
}

pub(crate) fn arriving<C: Labels>(path: &str, text: &str) -> Container {
    let mut rows = Rows::upto(BESIDE);
    let language = Language::from_path(path);
    let lines: Vec<(Ink, &str)> = text.lines().map(|line| (Ink::Arriving, line)).collect();
    let painted = code::highlighted_code(language, theme().syntax(), &lines);
    for ((_, line), spans) in lines.iter().zip(painted) {
        let line = (*line).to_owned();
        rows.push(move || marked("+", &line, Ink::Arriving, spans.clone()));
    }

    let total = rows.total();
    file::<C>(path.to_owned(), code::diff_stat(total, 0), rows)
}

fn file<C: Labels>(title: String, stat: impl View, rows: Rows) -> Container {
    let body = rows.into_body::<C>(Some(TALLEST));
    card(
        column! {
            heading(title, stat);
            if let Some(body) = body {
                rule();
                body;
            }
        },
        Inset::xy(0.0, 6.0),
        palette().outline,
    )
}

fn line_row(line: &Line, spans: Arc<[TextSpan]>) -> Element {
    match line {
        Line::Added(text) => marked("+", text, Ink::Arriving, spans),
        Line::Removed(text) => marked("-", text, Ink::Leaving, spans),
        Line::Context(text) => marked(" ", text, Ink::Kept, spans),
    }
}

fn inked(line: &Line) -> (Ink, &str) {
    match line {
        Line::Added(text) => (Ink::Arriving, text.as_str()),
        Line::Removed(text) => (Ink::Leaving, text.as_str()),
        Line::Context(text) => (Ink::Kept, text.as_str()),
    }
}

fn line_number(at: usize) -> Element {
    marked("@@", &format!("line {at}"), Ink::Placed, code::plain())
}

fn marked(mark: &'static str, text: &str, ink: Ink, spans: Arc<[TextSpan]>) -> Element {
    let color = match ink {
        Ink::Arriving | Ink::Leaving => palette().text,
        Ink::Kept | Ink::Placed => palette().text_muted,
    };
    let gutter = mono(mark, CODE_FONT_SIZE)
        .color(color)
        .no_wrap()
        .width(GUTTER);
    code::line(gutter, text, ink, spans)
}

fn heading(title: String, stat: impl View) -> impl View {
    row! {
        mono(title, CODE_FONT_SIZE).medium().color(palette().text).grow(1.0);
        stat;
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::edges(2.0, CARD_INSET, 8.0, CARD_INSET))
    .fill_width()
}
