use std::sync::Arc;

use rikka_syntax::Language;

use crate::card::CARD_INSET;
use crate::chat::Labels;
use crate::mono::{CODE_FONT_SIZE, LINE_HEIGHT_FACTOR, mono};
use crate::prelude::*;
use rikka_widgets::row;

pub(crate) const CHIP: f32 = 11.0;

const TINT: f32 = 0.18;

pub(crate) const TALLEST: f32 = 420.0;

pub const CODE_ROW_HEIGHT: f32 = 18.0;

pub(crate) const BESIDE: usize = 240;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ink {
    Arriving,
    Leaving,
    Kept,
    Placed,
}

#[derive(Clone, Copy)]
enum Side {
    Was,
    Is,
}

impl Ink {
    fn colors(self, palette: Palette) -> (Color, Color) {
        match self {
            Ink::Arriving => (palette.text, palette.surface.mix(palette.success, TINT)),
            Ink::Leaving => (palette.text, palette.surface.mix(palette.danger, TINT)),
            Ink::Kept => (palette.text_muted, Color::TRANSPARENT),
            Ink::Placed => (palette.text_muted, palette.surface_variant),
        }
    }

    fn stands_on(self, side: Side) -> bool {
        matches!(
            (self, side),
            (Ink::Kept, _) | (Ink::Leaving, Side::Was) | (Ink::Arriving, Side::Is)
        )
    }

    fn painted_on(self, side: Side) -> bool {
        matches!(
            (self, side),
            (Ink::Leaving, Side::Was) | (Ink::Kept | Ink::Arriving, Side::Is)
        )
    }
}

pub fn line(gutter: impl View, text: &str, ink: Ink, spans: Arc<[TextSpan]>) -> Element {
    let (color, ground) = ink.colors(palette());
    container(
        row! {
            gutter;
            code_text(text, color, spans).grow(1.0);
        }
        .align_items(Align::Start)
        .fill_width(),
    )
    .background(ground)
    .padding(Inset::xy(CARD_INSET, 0.0))
    .fill_width()
    .any()
}

fn code_text(text: &str, color: Color, spans: Arc<[TextSpan]>) -> RichText {
    rich_text(text.to_owned())
        .spans(spans)
        .family(Family::parse(font::JETBRAINS_MONO))
        .text_size(CODE_FONT_SIZE)
        .line_height((CODE_FONT_SIZE * LINE_HEIGHT_FACTOR).round())
        .color(color)
}

pub(crate) fn plain() -> Arc<[TextSpan]> {
    Arc::default()
}

pub fn highlighted_code(
    language: Option<Language>,
    colors: Syntax,
    rows: &[(Ink, &str)],
) -> Vec<Arc<[TextSpan]>> {
    let mut painted = vec![plain(); rows.len()];
    let Some(language) = language else {
        return painted;
    };
    for side in [Side::Was, Side::Is] {
        if !rows.iter().any(|(ink, _)| ink.painted_on(side)) {
            continue;
        }
        let indices: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, (ink, _))| ink.stands_on(side))
            .map(|(at, _)| at)
            .collect();
        let mut text = String::new();
        let mut starts = Vec::with_capacity(indices.len());
        for at in &indices {
            starts.push(text.len());
            text.push_str(rows[*at].1);
            text.push('\n');
        }
        let spans = language.spans(&text, colors);
        let mut position = 0;
        for (row, start) in indices.into_iter().zip(starts) {
            let end = start + rows[row].1.len();
            let mut merged: Vec<TextSpan> = Vec::new();
            while let Some(span) = spans.get(position) {
                if span.range.start >= end {
                    break;
                }
                let from = span.range.start.max(start);
                let upto = span.range.end.min(end);
                if from < upto {
                    merged.push(TextSpan {
                        range: from - start..upto - start,
                        ..span.clone()
                    });
                }
                match span.range.end > end {
                    true => break,
                    false => position += 1,
                }
            }
            if rows[row].0.painted_on(side) {
                painted[row] = merged.into();
            }
        }
    }
    painted
}

type Row = Box<dyn Fn() -> Element>;

pub(crate) struct Rows {
    visible_rows: Vec<Row>,
    rest: usize,
    most: usize,
}

impl Rows {
    pub(crate) fn upto(most: usize) -> Rows {
        Rows {
            visible_rows: Vec::new(),
            rest: 0,
            most,
        }
    }

    pub(crate) fn push<V: View>(&mut self, row: impl Fn() -> V + 'static) {
        match self.visible_rows.len() < self.most {
            true => self.visible_rows.push(Box::new(move || row().build())),
            false => self.rest += 1,
        }
    }

    pub(crate) fn total(&self) -> usize {
        self.visible_rows.len() + self.rest
    }

    pub(crate) fn into_body<C: Labels>(mut self, tallest: Option<f32>) -> Option<Element> {
        if self.visible_rows.is_empty() {
            return None;
        }
        let rest = self.rest;
        if rest > 0 {
            self.visible_rows
                .push(Box::new(move || folded::<C>(rest).build()));
        }

        let rows = self.visible_rows;
        let lines = lazy_column(rows.len(), move |slot| rows[slot.index]())
            .estimate(CODE_ROW_HEIGHT)
            .fill_width();
        Some(match tallest {
            Some(tallest) => lines.max_height(tallest).any(),
            None => lines.grow(1.0).any(),
        })
    }
}

pub fn diff_stat(added: usize, removed: usize) -> Flex {
    row_of(
        diff_counts(added, removed)
            .map(|(label, color)| mono(label, CHIP).medium().color(color).no_wrap()),
    )
    .gap(Space::SNUG)
    .align_items(Align::Center)
}

pub fn diff_counts(added: usize, removed: usize) -> impl Iterator<Item = (String, Color)> {
    let palette = palette();
    let added = (added > 0).then(|| (format!("+{added}"), palette.success));
    let removed = (removed > 0).then(|| (format!("-{removed}"), palette.danger));
    added.into_iter().chain(removed)
}

fn folded<C: Labels>(rest: usize) -> Container {
    container(text(C::more_lines(rest)).caption().muted())
        .padding(Inset::edges(6.0, CARD_INSET, 2.0, CARD_INSET))
        .fill_width()
}
