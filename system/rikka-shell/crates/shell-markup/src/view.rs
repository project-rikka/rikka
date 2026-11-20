use rikka_types::{Color, Key, Modifiers, Size};
use rikka_ui::{
    Align, Element, Inset, Length, MaterialSymbol, Space, Styled, TextStyled, View, palette, text,
    tr,
};
use rikka_widgets::{ControlSize, button, container, icon, row, row_of, segmented, spacer};
use shell_kit::{
    EDITOR_EDGE, EDITOR_GAP, EDITOR_ICON, EDITOR_WIDTH, editor_error, editor_loading, editor_window,
};

use crate::canvas::Canvas;
use crate::editor::Editor;
use crate::tools::{INKS, Pick, Width};

const TOOLS: f32 = 280.0;
const WIDTHS: f32 = 168.0;

const SWATCH: f32 = 18.0;
const SWATCH_RADIUS: f32 = 9.0;
const SWATCH_BORDER: f32 = 2.0;

const MIN_CANVAS: f32 = 220.0;
const MAX_CANVAS: f32 = 620.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Chord {
    Close,
    Undo,
    Redo,
    Save,
    Copy,
}

pub(crate) fn view(editor: &Editor) -> impl View {
    editor_window(
        editor.spot(),
        MaterialSymbol::DRAW,
        tr!("shell.markup.title"),
        Editor::close,
    )
    .on_key(on_key)
    .tools(tools(editor))
    .body(drawing(editor))
}

fn on_key(editor: &mut Editor, key: Key, modifiers: Modifiers) -> bool {
    let Some(chord) = chord(key, modifiers) else {
        return false;
    };
    match chord {
        Chord::Close => editor.close(),
        Chord::Undo => editor.undo(),
        Chord::Redo => editor.redo(),
        Chord::Save => editor.save(),
        Chord::Copy => editor.copy(),
    }
    true
}

fn chord(key: Key, modifiers: Modifiers) -> Option<Chord> {
    if key == Key::Escape {
        return Some(Chord::Close);
    }
    if !modifiers.control {
        return None;
    }
    match key.to_lowercase() {
        Key::Char('z') if modifiers.shift => Some(Chord::Redo),
        Key::Char('z') => Some(Chord::Undo),
        Key::Char('y') => Some(Chord::Redo),
        Key::Char('s') => Some(Chord::Save),
        Key::Char('c') => Some(Chord::Copy),
        _ => None,
    }
}

fn tools(editor: &Editor) -> Element {
    let picked = editor.picked();
    let drawable = editor.shown().is_some();

    row! {
        picks(picked.pick, drawable);
        widths(picked.width, drawable);
        inks(picked.ink, drawable);
        spacer().grow(1.0);
        button(icon(MaterialSymbol::UNDO).extent(Length::px(EDITOR_ICON)))
            .ghost()
            .control_size(ControlSize::Small)
            .icon_only()
            .enabled(editor.can_undo())
            .on_press(Editor::undo);
        button(icon(MaterialSymbol::REDO).extent(Length::px(EDITOR_ICON)))
            .ghost()
            .control_size(ControlSize::Small)
            .icon_only()
            .enabled(editor.can_redo())
            .on_press(Editor::redo);
        button(icon(MaterialSymbol::CONTENT_COPY).extent(Length::px(EDITOR_ICON)))
            .ghost()
            .control_size(ControlSize::Small)
            .icon_only()
            .enabled(drawable && !editor.is_working())
            .on_press(Editor::copy);
        button(text(tr!("shell.markup.save")).caption())
            .control_size(ControlSize::Small)
            .enabled(drawable && !editor.is_working() && editor.is_unsaved())
            .on_press(Editor::save)
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::xy(EDITOR_EDGE, EDITOR_GAP))
    .fill_width()
    .build()
}

fn picks(picked: Pick, drawable: bool) -> impl View {
    segmented(picked.index())
        .segments(Pick::ALL.map(Pick::label))
        .glyphs(Pick::ALL.map(Pick::symbol))
        .control_size(ControlSize::Small)
        .width(TOOLS)
        .enabled(drawable)
        .on_select(|editor: &mut Editor, at: usize| {
            editor.pick(Pick::from_index(at).unwrap_or_default());
        })
}

fn widths(picked: Width, drawable: bool) -> impl View {
    segmented(picked.index())
        .segments(Width::ALL.map(Width::label))
        .control_size(ControlSize::Small)
        .width(WIDTHS)
        .enabled(drawable)
        .on_select(|editor: &mut Editor, at: usize| {
            editor.widen(Width::from_index(at).unwrap_or_default());
        })
}

fn inks(picked: Color, drawable: bool) -> impl View {
    let palette = palette();
    row_of(INKS.into_iter().map(|ink| {
        let edge = match ink == picked {
            true => palette.accent,
            false => palette.outline,
        };
        let swatch = container(spacer())
            .background(ink)
            .border(SWATCH_BORDER, edge)
            .radius(SWATCH_RADIUS)
            .width(SWATCH)
            .height(SWATCH)
            .focusless();
        match drawable {
            true => swatch.on_press(move |editor: &mut Editor| editor.ink(ink)),
            false => swatch,
        }
    }))
    .gap(Space::TIGHT)
    .align_items(Align::Center)
}

fn drawing(editor: &Editor) -> Element {
    if let Some(trouble) = editor.trouble() {
        return editor_error(trouble, MIN_CANVAS).build();
    }
    let Some(shown) = editor.shown() else {
        return editor_loading(MIN_CANVAS).build();
    };

    let natural = shown.natural();
    container(
        Canvas {
            picture: shown.source.clone(),
            natural,
            sketch: editor.sketch(),
            picked: editor.picked(),
        }
        .build()
        .fill_width()
        .fill_height(),
    )
    .padding(Space::NORMAL)
    .height(tall(natural))
    .fill_width()
    .build()
}

fn tall(natural: Size) -> f32 {
    if natural.is_empty() {
        return MIN_CANVAS;
    }
    let across = EDITOR_WIDTH - EDITOR_EDGE * 2.0;
    (across * natural.height / natural.width + EDITOR_EDGE * 2.0).clamp(MIN_CANVAS, MAX_CANVAS)
}
