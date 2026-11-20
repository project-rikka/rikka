use rikka_theme::Corner;
use rikka_types::{Color, Key, Modifiers};
use rikka_ui::{
    Align, Element, ImageSource, Length, MaterialSymbol, Space, Styled, TextStyled, View, palette,
    text, tr,
};
use rikka_widgets::{
    ImageFit, button, checkbox, column, container, grid_of, icon, image, page_stack, row,
    scrollable, segmented, spacer, spinner,
};

use crate::picker::sources::SourceEntry;
use crate::picker::window::Picker;
use crate::protocol::{Persist, SourceKind};

const SYMBOL: f32 = 26.0;
const EMPTY_SYMBOL: f32 = 34.0;

const PREVIEW_HEIGHT: f32 = 148.0;

const ACROSS: usize = 3;

pub(crate) fn view(picker: &Picker) -> impl View {
    let body = column! {
        heading(picker);
        if picker.kinds().len() > 1 {
            tabs(picker);
        }
        page_stack(picker.tab(), stage(picker)).grow(1.0).fill_width();
        footer(picker)
    }
    .gap(Space::ROOMY)
    .padding(Space::LOOSE)
    .fill();

    container(body)
        .fill()
        .on_chord(Key::Escape, Modifiers::NONE, |picker: &mut Picker| {
            picker.cancel()
        })
}

fn heading(picker: &Picker) -> impl View {
    let subtitle = match picker.request().app_id.is_empty() {
        true => tr!("cast.subtitle-unknown"),
        false => tr!("cast.subtitle", app = &picker.request().app_id),
    };
    let heading = column! {
        text(tr!("cast.title")).heading().semibold();
        text(subtitle).label().muted();
        if picker.request().multiple {
            text(tr!("cast.hint-multiple")).caption().muted();
        }
    }
    .gap(Space::HAIR);

    row! {
        icon(MaterialSymbol::SCREEN_SHARE)
            .extent(Length::px(SYMBOL))
            .color(palette().accent);
        heading.grow(1.0)
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
}

fn tabs(picker: &Picker) -> impl View {
    let labels = picker.kinds().iter().map(|kind| match kind {
        SourceKind::Monitor => tr!("cast.tab.monitors"),
        SourceKind::Window => tr!("cast.tab.windows"),
    });
    segmented(picker.tab())
        .segments(labels)
        .on_select(|picker: &mut Picker, tab: usize| picker.select_tab(tab))
}

fn stage(picker: &Picker) -> Element {
    if picker.is_loading() {
        return centred(
            column! {
                spinner().extent(Length::px(28.0));
                text(tr!("cast.looking")).label().muted()
            }
            .gap(Space::NORMAL)
            .align_items(Align::Center),
        );
    }

    if let Some(failure) = picker.failure() {
        return centred(
            column! {
                icon(MaterialSymbol::ERROR)
                    .extent(Length::px(EMPTY_SYMBOL))
                    .color(palette().danger);
                text(tr!("cast.failure", reason = failure)).label().muted()
            }
            .gap(Space::NORMAL)
            .align_items(Align::Center),
        );
    }

    let visible_sources = picker.visible_sources();
    if visible_sources.is_empty() {
        let (symbol, message) = match picker.shows_windows() {
            true => (MaterialSymbol::DESKTOP_WINDOWS, tr!("cast.empty.windows")),
            false => (MaterialSymbol::MONITOR, tr!("cast.empty.monitors")),
        };
        return centred(
            column! {
                icon(symbol)
                    .extent(Length::px(EMPTY_SYMBOL))
                    .color(palette().text_muted);
                text(message).label().muted()
            }
            .gap(Space::NORMAL)
            .align_items(Align::Center),
        );
    }

    let cards = visible_sources.iter().map(|entry| card(picker, entry));
    let across = visible_sources.len().clamp(1, ACROSS);
    scrollable(grid_of(across, cards).gap(Space::NORMAL).fill_width())
        .grow(1.0)
        .build()
}

fn card(picker: &Picker, entry: &SourceEntry) -> impl View {
    let palette = palette();
    let selected = picker.is_selected(&entry.source);

    let title = match entry.title.is_empty() {
        true => text(match entry.source.kind {
            SourceKind::Monitor => tr!("cast.monitor-unnamed"),
            SourceKind::Window => tr!("cast.window-unnamed"),
        }),
        false => text(entry.title.clone()),
    };
    let heading = column! {
        title.label().medium().no_wrap();
        if !entry.detail.is_empty() {
            text(entry.detail.clone()).caption().muted().no_wrap();
        }
    }
    .gap(Space::HAIR);

    let named = row! {
        heading.grow(1.0);
        if selected {
            icon(MaterialSymbol::CHECK_CIRCLE)
                .extent(Length::px(18.0))
                .color(palette.accent);
        }
    }
    .gap(Space::SNUG)
    .align_items(Align::Center);

    let (fill, edge, width) = match selected {
        true => (palette.selection, palette.accent, 2.0),
        false => (palette.surface_variant, palette.outline, 1.0),
    };
    let source = entry.source.clone();
    container(
        column! {
            preview(entry);
            named
        }
        .gap(Space::SNUG)
        .padding(Space::SNUG)
        .fill_width(),
    )
    .background(fill)
    .border(width, edge)
    .radius(Corner::Panel)
    .on_press(move |picker: &mut Picker| picker.toggle(source.clone()))
}

fn preview(entry: &SourceEntry) -> impl View {
    let inside = match &entry.preview {
        Some(preview) => image(ImageSource::Rgba {
            width: preview.width,
            height: preview.height,
            pixels: preview.pixels.clone(),
        })
        .fit(ImageFit::Contain)
        .fill()
        .build(),
        None => waiting(entry).build(),
    };
    container(inside)
        .background(Color::rgba(0.0, 0.0, 0.0, 0.28))
        .radius(Corner::Item)
        .height(PREVIEW_HEIGHT)
        .fill_width()
        .center()
}

fn waiting(entry: &SourceEntry) -> impl View {
    let symbol = match entry.source.kind {
        SourceKind::Monitor => MaterialSymbol::MONITOR,
        SourceKind::Window => MaterialSymbol::DESKTOP_WINDOWS,
    };
    icon(symbol)
        .extent(Length::px(EMPTY_SYMBOL))
        .color(palette().text_muted)
}

fn footer(picker: &Picker) -> impl View {
    row! {
        if picker.request().persist.is_offered() {
            let message = match picker.request().persist {
                Persist::UntilRevoked => tr!("cast.remember"),
                _ => tr!("cast.remember-session"),
            };
            checkbox(picker.is_remembered())
                .label(text(message).label().muted())
                .on_toggle(|picker: &mut Picker, remember: bool| picker.remember(remember));
        }
        spacer().grow(1.0);
        button(text(tr!("cast.cancel")))
            .ghost()
            .on_press(|picker: &mut Picker| picker.cancel());
        button(text(tr!("cast.share")))
            .primary()
            .enabled(picker.has_selection())
            .on_press(|picker: &mut Picker| picker.share())
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
}

fn centred(child: impl View) -> Element {
    container(child).center().grow(1.0).build()
}
