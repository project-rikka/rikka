use rikka_llm::Value;
use rikka_thread::{AnsweredQuestion, Answers};

use crate::card::{CARD_INSET, card};
use crate::prelude::*;
use crate::question;
use rikka_widgets::{column, row};

const GLYPH: f32 = 14.0;

pub(crate) fn view(questions: &[Value], output: Option<&str>) -> impl View {
    let answers = Answers::read(output.unwrap_or_default());

    card(
        column! {
            for (at, question) in questions.iter().enumerate() {
                if at > 0 {
                    rule();
                }
                put(question, answers.as_slice().get(at));
            }
        },
        Inset::NONE,
        palette().outline,
    )
}

fn put(question: &Value, answer_entry: Option<&AnsweredQuestion>) -> impl View {
    let palette = palette();
    let header = string_field(question, "header");

    column! {
        row! {
            if !header.trim().is_empty() {
                question::question_chip(header);
            }
            text(string_field(question, "question").to_owned())
                .label()
                .color(palette.text)
                .grow(1.0);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
        if let Some(selection) = answer_entry.map(AnsweredQuestion::selection) {
            for label in selection.labels() {
                answer(MaterialSymbol::CHECK, label, palette.text);
            }
            if let Some(wrote) = selection.text() {
                answer(MaterialSymbol::EDIT, wrote, palette.text_muted);
            }
        }
    }
    .gap(Space::TIGHT)
    .padding(Inset::xy(CARD_INSET, 8.0))
    .fill_width()
}

fn answer(symbol: MaterialSymbol, label: &str, colour: Color) -> impl View {
    row! {
        icon(symbol).extent(Length::px(GLYPH)).color(palette().accent);
        text(label.to_owned()).label().color(colour).grow(1.0);
    }
    .gap(Space::SNUG)
    .align_items(Align::Start)
    .fill_width()
}

fn string_field<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}
