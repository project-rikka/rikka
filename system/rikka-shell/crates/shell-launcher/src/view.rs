use rikka_theme::Corner;
use rikka_types::{Key, Modifiers};
use rikka_ui::{
    Align, Curve, DurationToken, Easing, Element, IconSource, Inset, Justify, Label, Length,
    MaterialSymbol, Motion, Property, Space, Spring, Styled, TextStyled, View, fade, is_exiting,
    offset, palette, scale, text, tr,
};
use rikka_widgets::{
    ControlSize, column, container, icon, list, list_item, row, spacer, text_field,
};
use shell_kit::{scrim, sheet};

use crate::gallery::{self, Tile};
use crate::provider::{Layout, Preview};
use crate::state::Launcher;

const WIDTH: f32 = 720.0;
const DOWN: f32 = 20.0;

const BORDER: f32 = 1.0;

const SEARCH_HEIGHT: f32 = 60.0;
const QUERY_SIZE: f32 = 22.0;
const SEARCH_GAP: f32 = 14.0;
const SEARCH_ICON: f32 = 24.0;

const RESULTS_HEIGHT: f32 = 336.0;

const RESULTS: &str = "launcher.results";

const ENTER_FROM: f32 = 0.93;

const CHIP_FROM: f32 = 0.85;

const LIFT: f32 = 10.0;

const ARRIVAL: Curve = Curve::Tween {
    easing: Easing::EmphasizedDecel,
    duration: DurationToken::Short4,
};

const PAGE: i32 = 10;

pub(crate) const EXIT_DURATION: DurationToken = DurationToken::Short4;

pub(crate) fn view(launcher: &Launcher) -> impl View {
    scrim(
        column! {
            spacer().height(Length::portion(DOWN / 100.0));
            if !is_exiting() {
                sheet(contents(launcher), on_key)
                    .keeps_tab()
                    .build()
                    .width(WIDTH)
                    .max_width(Length::portion(0.94))
                    .transition(
                        (fade() + scale(ENTER_FROM))
                            .enter(Curve::Spring(Spring::DEFAULT_SPATIAL))
                            .exit(Motion::exit(EXIT_DURATION).curve(Property::Effects)),
                    );
            }
        }
        .align_items(Align::Center)
        .fill(),
        Launcher::escape,
    )
}

fn on_key(launcher: &mut Launcher, key: Key, modifiers: Modifiers) -> bool {
    let control = modifiers.control;
    let across = launcher.layout() == Layout::Gallery;
    let delta = match key {
        Key::ArrowDown => 1,
        Key::ArrowUp => -1,
        Key::ArrowRight if across => 1,
        Key::ArrowLeft if across => -1,
        Key::PageDown => PAGE,
        Key::PageUp => -PAGE,
        Key::Escape => {
            launcher.escape();
            return true;
        }
        Key::Tab if !modifiers.shift => {
            launcher.complete();
            return true;
        }
        Key::Char('n') if control => 1,
        Key::Char('p') if control => -1,
        _ => return false,
    };
    launcher.walk(delta);
    true
}

fn contents(launcher: &Launcher) -> impl View {
    let palette = palette();
    let showing = (
        RESULTS,
        launcher.layout(),
        launcher.triggered(),
        launcher.shown().next().is_some(),
    );
    container(
        column! {
            search(launcher);
            if let Some(results) = results(launcher) {
                results
                    .fill_width()
                    .transition((fade() + offset(0.0, LIFT)).enter(ARRIVAL))
                    .keyed(showing);
            }
        }
        .fill_width()
        .padding(Space::SNUG),
    )
    .background(palette.background)
    .border(BORDER, palette.outline)
    .radius(Corner::Window)
    .glass()
    .resizing()
}

fn search(launcher: &Launcher) -> impl View {
    let selected = launcher.selected();
    row! {
        icon(MaterialSymbol::SEARCH)
            .extent(Length::px(SEARCH_ICON))
            .color(palette().text_muted);
        text_field(launcher.field())
            .placeholder(tr!("shell.launcher.search"))
            .control_size(ControlSize::Large)
            .font_size(QUERY_SIZE)
            .plain(true)
            .leaves_arrows(launcher.layout() == Layout::Gallery)
            .autofocus(true)
            .on_change(Launcher::search)
            .on_submit(move |launcher: &mut Launcher| launcher.activate(selected))
            .grow(1.0);
        if let Some(index) = launcher.triggered()
            && let Some(info) = launcher.provider(index)
        {
            chip(info.label())
                .transition((fade() + scale(CHIP_FROM)).enter(ARRIVAL))
                .keyed(index);
        }
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .padding(Inset::xy(SEARCH_GAP, 0.0))
    .height(SEARCH_HEIGHT)
}

fn chip(label: Label) -> impl View {
    let palette = palette();
    container(text(label).caption().color(palette.on_accent).medium())
        .background(palette.accent)
        .radius(Corner::Item)
        .padding(Inset::xy(8.0, 3.0))
}

fn results(launcher: &Launcher) -> Option<Element> {
    if launcher.shown().next().is_none() {
        return (!launcher.field().is_empty()).then(|| nothing_found().build());
    }
    match launcher.layout() {
        Layout::List => Some(lines(launcher).build()),
        Layout::Gallery => Some(pictures(launcher).build()),
    }
}

fn pictures(launcher: &Launcher) -> impl View {
    let tiles = launcher
        .shown()
        .map(|ranked| Tile {
            label: Label::from(ranked.hit.title()),
            preview: ranked
                .hit
                .picture()
                .cloned()
                .unwrap_or(Preview::Still(None)),
            in_use: ranked.hit.is_in_use(),
        })
        .collect();
    gallery::gallery(
        tiles,
        launcher.selected(),
        |launcher: &mut Launcher, index| launcher.select(index),
        |launcher: &mut Launcher, index| launcher.activate(index),
    )
    .turned(launcher.turn())
}

fn lines(launcher: &Launcher) -> impl View {
    let palette = palette();
    let rows = launcher.shown().map(|ranked| {
        let hit = &ranked.hit;
        let drawn = match hit.is_in_use() {
            true => IconSource::from(MaterialSymbol::CHECK),
            false => hit.drawn_as().cloned().unwrap_or_else(|| {
                launcher
                    .provider(ranked.provider)
                    .map_or(MaterialSymbol::SEARCH, |info| info.default_icon())
                    .into()
            }),
        };
        let item = list_item(hit.title())
            .keyed((ranked.provider, hit.key()))
            .icon(drawn)
            .tint(palette.accent);
        match hit.second_line() {
            Some(subtitle) => item.secondary(subtitle),
            None => item,
        }
    });

    list(rows)
        .selected(launcher.selected())
        .control_size(ControlSize::Large)
        .glides(false)
        .activates_on_press(true)
        .on_select(|launcher: &mut Launcher, index| launcher.select(index))
        .on_activate(|launcher: &mut Launcher, index| launcher.activate(index))
        .max_height(RESULTS_HEIGHT)
}

fn nothing_found() -> impl View {
    row! {
        icon(MaterialSymbol::SEARCH_OFF)
            .extent(Length::px(16.0))
            .color(palette().text_muted);
        text(tr!("shell.launcher.empty")).label().muted()
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .justify(Justify::Center)
    .padding(Inset::xy(SEARCH_GAP, 18.0))
    .fill_width()
}
