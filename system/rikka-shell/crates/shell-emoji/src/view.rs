use rikka_emoji::Tone;
use rikka_theme::{Corner, Palette};
use rikka_types::{Choice, Key, Modifiers};
use rikka_ui::{
    Align, Element, Inset, Label, MaterialSymbol, Space, Styled, TextStyled, View, palette, text,
    tr,
};
use rikka_widgets::{
    ButtonVariant, ControlSize, button, column, container, lazy_grid, page_stack, row, segmented,
    spacer, text_field,
};
use shell_kit::{finder, nothing, popover};

use crate::picker::Picker;
use crate::tab::Tab;

const WIDTH: f32 = 356.0;

const COLUMNS: usize = 9;
const CELL: f32 = 34.0;
const CELL_GAP: f32 = 2.0;
const GRID_ROWS: f32 = 8.0;
const GRID_HEIGHT: f32 = GRID_ROWS * (CELL + CELL_GAP);

const BORDER: f32 = 1.0;
const INSET: f32 = 10.0;

const HEADING_HEIGHT: f32 = ControlSize::Small.height();
const NAME_HEIGHT: f32 = 20.0;

const GRID: &str = "emoji.grid";

pub(crate) fn view(picker: &Picker) -> impl View {
    let selected = picker.selected();
    let tab = picker.tab();

    popover(contents(picker), |picker: &mut Picker| picker.close())
        .width(WIDTH)
        .beside(picker.aim())
        .keeps_tab()
        .on_key(move |picker: &mut Picker, key: Key, modifiers: Modifiers| {
            on_key(picker, key, modifiers, selected, tab)
        })
}

const PAGE: i32 = COLUMNS as i32 * 4;

fn on_key(picker: &mut Picker, key: Key, modifiers: Modifiers, selected: usize, tab: Tab) -> bool {
    match key.to_lowercase() {
        Key::Tab => picker.show(beside(tab, !modifiers.shift)),
        Key::ArrowRight => picker.walk(1),
        Key::ArrowLeft => picker.walk(-1),
        Key::ArrowDown => picker.walk(COLUMNS as i32),
        Key::ArrowUp => picker.walk(-(COLUMNS as i32)),
        Key::PageDown => picker.walk(PAGE),
        Key::PageUp => picker.walk(-PAGE),
        Key::Enter => picker.activate(selected),
        Key::Escape => picker.escape(),
        _ => return false,
    }
    true
}

fn beside(tab: Tab, forward: bool) -> Tab {
    let all = Tab::all();
    let at = all.iter().position(|listed| *listed == tab).unwrap_or(0);
    let next = match forward {
        true => (at + 1) % all.len(),
        false => (at + all.len() - 1) % all.len(),
    };
    all[next]
}

fn contents(picker: &Picker) -> impl View {
    let palette = palette();
    let contents = column! {
        heading(picker);
        search(picker);
        tabs(picker);
        page_stack(at(picker), cells(picker).keyed(GRID)).fill_width();
        naming(picker)
    }
    .fill_width()
    .gap(Space::TIGHT)
    .padding(Space::TIGHT);

    container(contents)
        .background(palette.surface)
        .border(BORDER, palette.outline)
        .radius(Corner::Window)
        .glass()
        .resizing()
}

fn heading(picker: &Picker) -> impl View {
    row! {
        text(tr!("shell.emoji.title")).caption().muted().medium();
        if picker.tab() == Tab::Recent && picker.has_recents() {
            button(text(tr!("shell.emoji.clear-recent")).caption())
                .variant(ButtonVariant::Ghost)
                .control_size(ControlSize::Small)
                .on_press(Picker::forget);
        }
        spacer().grow(1.0);
        if picker.takes_tone() {
            swatches(picker);
        }
    }
    .align_items(Align::Center)
    .gap(Space::TIGHT)
    .padding(Inset::xy(INSET, 0.0))
    .height(HEADING_HEIGHT)
}

fn swatches(picker: &Picker) -> impl View {
    row! {
        for &tone in Tone::ALL {
            swatch(picker, tone);
        }
    }
    .align_items(Align::Center)
}

fn swatch(picker: &Picker, tone: Tone) -> impl View {
    let swatch = container(text(picker.swatch(tone)).body())
        .padding(Space::HAIR)
        .radius(Corner::Item)
        .focusless()
        .on_press(move |picker: &mut Picker| picker.set_tone(tone));
    match picker.tone() == tone {
        true => swatch.background(palette().selection),
        false => swatch,
    }
}

fn search(picker: &Picker) -> impl View {
    let selected = picker.selected();
    finder(
        text_field(picker.field())
            .placeholder(tr!("shell.emoji.search"))
            .leaves_arrows(true)
            .on_change(Picker::narrow)
            .on_submit(move |picker: &mut Picker| picker.activate(selected)),
    )
}

fn at(picker: &Picker) -> usize {
    Tab::all()
        .iter()
        .position(|tab| *tab == picker.tab())
        .unwrap_or_default()
}

fn tabs(picker: &Picker) -> impl View {
    let all = Tab::all();
    let labels: Vec<Label> = all.iter().map(|tab| tab.label()).collect();
    let glyphs: Vec<MaterialSymbol> = all.iter().map(|tab| tab.symbol()).collect();

    segmented(at(picker))
        .segments(labels)
        .glyphs(glyphs)
        .control_size(ControlSize::Small)
        .focusless()
        .on_select(move |picker: &mut Picker, at: usize| picker.show(all[at]))
}

fn cells(picker: &Picker) -> Element {
    if picker.count() == 0 {
        return match picker.is_searching() {
            true => nothing(MaterialSymbol::SEARCH_OFF, tr!("shell.emoji.no-match")).build(),
            false => nothing(MaterialSymbol::HISTORY, tr!("shell.emoji.no-recent")).build(),
        };
    }
    let palette = palette();
    let cells = picker.cells();
    let count = cells.len();
    let selected = picker.selected();

    lazy_grid(COLUMNS, count, move |slot| {
        cell(
            cells.get(slot.index),
            slot.index,
            slot.index == selected,
            palette,
        )
    })
    .uniform(CELL)
    .gap(Space::HAIR)
    .focus(selected)
    .shows(picker.listing())
    .height(spanned(count))
    .build()
}

fn spanned(cells: usize) -> f32 {
    let rows = cells.div_ceil(COLUMNS) as f32;
    (rows * (CELL + CELL_GAP)).min(GRID_HEIGHT)
}

fn cell(shown: &'static str, at: usize, selected: bool, palette: Palette) -> impl View {
    let cell = container(text(shown).title())
        .center()
        .radius(Corner::Item)
        .height(CELL)
        .focusless()
        .on_press(move |picker: &mut Picker| picker.activate(at));
    match selected {
        true => cell.background(palette.selection),
        false => cell,
    }
}

fn naming(picker: &Picker) -> impl View {
    let named = picker.showing().map(|emoji| emoji.name).unwrap_or_default();

    row! {
        text(named).caption().muted().no_wrap()
    }
    .align_items(Align::Center)
    .padding(Inset::xy(INSET, 0.0))
    .height(NAME_HEIGHT)
}
