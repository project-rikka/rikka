use std::time::Duration;

use rikka_theme::Corner;
use rikka_types::{Color, Key, Modifiers, Size};
use rikka_ui::{
    Align, Element, Inset, Length, MaterialSymbol, Space, Styled, TextStyled, View, text, tr,
};
use rikka_widgets::{ControlSize, ImageFit, button, column, container, icon, row, spacer, video};
use shell_kit::{
    EDITOR_EDGE, EDITOR_GAP, EDITOR_ICON, EDITOR_WIDTH, editor_error, editor_loading, editor_window,
};

use crate::cut::Cut;
use crate::editor::{Editor, Step};
use crate::preview::Preview;
use crate::timeline::{self, Timeline};

const MIN_STAGE: f32 = 200.0;
const MAX_STAGE: f32 = 460.0;

pub(crate) fn view(editor: &Editor) -> impl View {
    editor_window(
        editor.spot(),
        MaterialSymbol::CONTENT_CUT,
        match editor.is_draft() {
            true => tr!("shell.trim.replay"),
            false => tr!("shell.trim.title"),
        },
        Editor::close,
    )
    .on_key(on_key)
    .tools(tools(editor))
    .body(stage(editor))
}

fn on_key(editor: &mut Editor, key: Key, modifiers: Modifiers) -> bool {
    if key == Key::Escape {
        editor.close();
        return true;
    }
    if modifiers.control {
        return match key.to_lowercase() {
            Key::Char('s') => {
                editor.save();
                true
            }
            _ => false,
        };
    }
    match key.to_lowercase() {
        Key::Space => editor.play_pause(),
        Key::ArrowLeft => editor.nudged(Step::Back),
        Key::ArrowRight => editor.nudged(Step::On),
        _ => return false,
    }
    true
}

fn tools(editor: &Editor) -> Element {
    let cuttable = editor.is_ready() && !editor.is_working();
    let playing = editor.preview().is_some_and(Preview::is_playing);
    let glyph = match playing {
        true => MaterialSymbol::PAUSE,
        false => MaterialSymbol::PLAY_ARROW,
    };

    row! {
        button(icon(glyph).extent(Length::px(EDITOR_ICON)).filled())
            .ghost()
            .control_size(ControlSize::Small)
            .icon_only()
            .enabled(cuttable)
            .on_press(Editor::play_pause);
        text(clock(editor))
            .caption()
            .muted()
            .no_wrap();
        spacer().grow(1.0);
        button(text(tr!("shell.trim.save")).caption())
            .control_size(ControlSize::Small)
            .loading(editor.is_working())
            .enabled(cuttable && editor.is_unsaved())
            .on_press(Editor::save)
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::xy(EDITOR_EDGE, EDITOR_GAP))
    .fill_width()
    .build()
}

fn stage(editor: &Editor) -> Element {
    if let Some(trouble) = editor.trouble() {
        return editor_error(trouble, MIN_STAGE).build();
    }
    let Some(preview) = editor.preview() else {
        return editor_loading(MIN_STAGE).build();
    };

    column! {
        container(video(preview.video()).fit(ImageFit::Contain).fill())
            .background(Color::BLACK)
            .radius(Corner::Panel)
            .fill_width()
            .height(tall(preview.shape()));
        track(editor)
    }
    .gap(Space::SNUG)
    .padding(Space::NORMAL)
    .fill_width()
    .build()
}

fn track(editor: &Editor) -> Element {
    let Some(cut) = editor.cut() else {
        return editor_loading(timeline::HEIGHT).build();
    };

    column! {
        Timeline {
            cut,
            position: editor.preview().map(Preview::position).unwrap_or_default(),
            keyframes: editor.keyframes().unwrap_or_default(),
            enabled: editor.is_ready() && !editor.is_working(),
        }
        .build()
        .fill_width()
        .height(timeline::HEIGHT);
        row! {
            text(spoken(cut.start()))
                .caption()
                .muted()
                .no_wrap();
            spacer().grow(1.0);
            text(spoken(cut.kept_duration()))
                .caption()
                .no_wrap();
            spacer().grow(1.0);
            text(spoken(cut.end()))
                .caption()
                .muted()
                .no_wrap()
        }
        .align_items(Align::Center)
        .fill_width()
    }
    .gap(Space::TIGHT)
    .fill_width()
    .build()
}

fn tall(shape: Option<Size>) -> f32 {
    let Some(shape) = shape.filter(|shape| !shape.is_empty()) else {
        return MIN_STAGE;
    };
    let across = EDITOR_WIDTH - EDITOR_EDGE * 2.0;
    (across * shape.height / shape.width).clamp(MIN_STAGE, MAX_STAGE)
}

fn clock(editor: &Editor) -> String {
    let position = editor.preview().map(Preview::position).unwrap_or_default();
    let length = editor.cut().map(Cut::length).unwrap_or_default();
    format!("{} / {}", spoken(position), spoken(length))
}

fn spoken(at: Duration) -> String {
    let tenths = at.as_millis() / 100;
    let (seconds, tenth) = (tenths / 10, tenths % 10);
    let (minutes, seconds) = (seconds / 60, seconds % 60);
    match minutes >= 60 {
        true => format!("{}:{:02}:{seconds:02}.{tenth}", minutes / 60, minutes % 60),
        false => format!("{minutes}:{seconds:02}.{tenth}"),
    }
}
