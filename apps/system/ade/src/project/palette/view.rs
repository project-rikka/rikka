use std::ops::Range;

use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::hint::hint;
use crate::project::act::{act, given};
use crate::project::glyph;
use crate::project::palette::state::{Choice, Palette};
use crate::project::state::Project;

pub(crate) const WIDTH: f32 = 560.0;
const ROW: f32 = 28.0;
const SHOWN: usize = 12;
const GLYPH: f32 = 16.0;
const FONT: f32 = 13.0;

pub(in crate::project) fn view(palette: &Palette) -> Element {
    popover(
        column! {
            text_field(palette.query())
                .placeholder(tr!("ade.palette.placeholder"))
                .autofocus(true)
                .claims_focus(true)
                .on_change(act(|project| project.palette_typed()))
                .on_submit(act(|project| project.take_palette(None)))
                .on_key(|project: &mut Project, key: Key, modifiers: Modifiers| {
                    project.palette_key(key, modifiers)
                })
                .fill_width();
            choices(palette);
        }
        .gap(8.0)
        .width(WIDTH),
    )
    .on_dismiss(act(|project| project.dismiss_palette()))
    .any()
}

fn choices(palette: &Palette) -> Element {
    let choices = palette.choices();
    if choices.is_empty() {
        return hint(tr!("ade.palette.nothing")).any();
    }
    let height = choices.len().min(SHOWN) as f32 * ROW;
    lazy_column(choices.len(), move |slot| choice_row(&choices[slot.index]))
        .uniform(ROW)
        .selected(Some(palette.chosen()))
        .reveals(Some(palette.chosen()))
        .lit(true)
        .activates_on_press(true)
        .on_activate(given(|project, index: usize| {
            project.take_palette(Some(index))
        }))
        .height(height)
        .fill_width()
        .any()
}

fn spans(ranges: impl Iterator<Item = Range<usize>>) -> Vec<TextSpan> {
    let accent = palette().accent;
    ranges
        .map(|range| TextSpan::new(range).color(accent).weight(font::BOLD))
        .collect()
}

fn choice_row(choice: &Choice) -> Element {
    let path_match = &choice.path_match;
    let leaf_start = path_match.leaf_start();
    let folder = path_match.under().trim_end_matches('/');
    let leaf = spans(
        path_match
            .matched()
            .iter()
            .filter(|range| range.end > leaf_start)
            .map(|range| range.start.max(leaf_start) - leaf_start..range.end - leaf_start),
    );
    let under = spans(
        path_match
            .matched()
            .iter()
            .filter(|range| range.start < folder.len())
            .map(|range| range.start..range.end.min(folder.len())),
    );
    let path = std::path::Path::new(path_match.path());
    row! {
        icon(glyph::file(path))
            .extent(Length::px(GLYPH))
            .color(palette().text_muted);
        rich_text(path_match.leaf()).spans(leaf).text_size(FONT).no_wrap();
        rich_text(folder)
            .spans(under)
            .caption()
            .muted()
            .no_wrap()
            .grow(1.0);
        if choice.recent {
            icon(MaterialSymbol::HISTORY)
                .extent(Length::px(14.0))
                .color(palette().text_muted);
        }
    }
    .gap(8.0)
    .align_items(Align::Center)
    .padding(Inset::xy(8.0, 0.0))
    .height(ROW)
    .fill_width()
    .any()
}
