use browser_store::Store;
use rikka_app::prelude::*;
use rikka_omnibox::Surface;
use rikka_widgets::row;

use crate::omnibox::listing::{Listing, Look, Row};
use crate::omnibox::running::Opening;
use crate::shell::Shell;
use crate::widgets::{favicon, glyph_button, stored_favicon};

const GLYPH: f32 = 18.0;
const SEPARATOR: &str = " — ";

pub(crate) fn rows(listing: &Listing, store: Option<&Store>) -> Element {
    column_of(listing.rows.iter().enumerate().map(|(index, row)| {
        let highlight = match (listing.selected, listing.hovered) {
            (Some(selected), _) if selected == index => Highlight::Selected,
            (_, Some(hovered)) if hovered == index => Highlight::Hovered,
            _ => Highlight::Plain,
        };
        line(listing.surface, index, row, highlight, store)
    }))
    .gap(Space::HAIR)
    .padding(Inset::all(Space::TIGHT))
    .fill_width()
    .any()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Highlight {
    Plain,
    Hovered,
    Selected,
}

fn line(
    surface: Surface,
    index: usize,
    row: &Row,
    highlight: Highlight,
    store: Option<&Store>,
) -> Element {
    let mut words = row.title.clone();
    let mut spans: Vec<TextSpan> = row
        .title_marks
        .iter()
        .map(|range| TextSpan::new(range.clone()).weight(Weight::BOLD))
        .collect();
    if !row.detail.is_empty() {
        words.push_str(SEPARATOR);
        let start = words.len();
        words.push_str(&row.detail);
        spans.insert(
            0,
            TextSpan::new(start - SEPARATOR.len()..words.len()).color(palette().text_muted),
        );
        spans.extend(row.detail_marks.iter().map(|range| {
            TextSpan::new(range.start + start..range.end + start)
                .weight(Weight::BOLD)
                .color(palette().text_muted)
        }));
    }
    let switch = row.switches.then(|| {
        container(
            text(tr!("browser.omnibox.switch"))
                .caption()
                .medium()
                .no_wrap(),
        )
        .padding(Inset::xy(Space::SNUG, Space::HAIR))
        .border(1.0, palette().outline)
        .radius(Corner::Item)
    });
    let remove = row.removable.then(|| {
        glyph_button(MaterialSymbol::CLOSE, 14.0)
            .on_press(move |shell: &mut Shell| shell.remove_suggestion(surface, index))
    });
    let ground = match highlight {
        Highlight::Selected => palette().accent.with_alpha(0.16),
        Highlight::Hovered => palette().text.with_alpha(0.06),
        Highlight::Plain => Color::TRANSPARENT,
    };
    container(
        row! {
            look(&row.look, store);
            rich_text(words)
                .spans(spans)
                .label()
                .no_wrap()
                .grow(1.0)
                .min_width(0.0);
            switch;
            remove;
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::NORMAL, Space::SNUG))
    .radius(Corner::Item)
    .background(ground)
    .on_press(move |shell: &mut Shell| shell.choose(surface, index, Opening::Here))
    .on_hover(move |shell: &mut Shell, inside: bool| shell.hover_row(surface, index, inside))
    .focusless()
    .fill_width()
    .build()
    .keyed((row.kind as u8, row.title.as_str(), row.fill.as_str()))
}

fn look(look: &Look, store: Option<&Store>) -> Element {
    match look {
        Look::Favicon(favicon_image) => favicon(Some(favicon_image), MaterialSymbol::PUBLIC, GLYPH),
        Look::Symbol(symbol) => icon(*symbol)
            .extent(Length::px(GLYPH))
            .color(palette().text_muted)
            .any(),
        Look::Stored(url) => stored_favicon(store, url, MaterialSymbol::PUBLIC, GLYPH),
    }
}
