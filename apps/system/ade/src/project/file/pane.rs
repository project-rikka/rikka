use std::path::Path;

use ade_git::State;
use rikka_app::prelude::*;
use rikka_chat::{CARD_INSET, CODE_FONT_SIZE, LINE_HEIGHT_FACTOR, mono};
use rikka_syntax::Language;
use rikka_ui::search::Action;
use rikka_widgets::{column, find_bar, find_mark, row};

use crate::hint::hint;
use crate::notice::notice;
use crate::project::act::{act, given};
use crate::project::file::state::{Answer, Shown, Viewer};
use crate::project::file::strip;
use crate::project::git::{self, Git};

const GLYPH: f32 = 14.0;
const DOT: f32 = 8.0;

const FIND: &str = "ade-file-find";

pub(in crate::project) fn view(viewer: &Viewer, root: &Path, git: &Git) -> Element {
    let Some(shown) = viewer.shown() else {
        return Element::empty();
    };
    column! {
        strip::view(viewer);
        rule();
        if let Some(asking) = viewer.asking() {
            closing_asked(asking, root);
            rule();
        }
        heading(shown, root, git);
        body(shown);
    }
    .grow(1.0)
    .fill_width()
    .any()
}

fn closing_asked(shown: &Shown, root: &Path) -> impl View {
    let answer = |label: Label, answer: Answer, secondary: bool| {
        let pressed = button(text(label).caption().no_wrap())
            .small()
            .on_press(act(move |project| {
                project.viewer(|viewer, git| viewer.answer(answer, git))
            }));
        match secondary {
            true => pressed.secondary(),
            false => pressed,
        }
    };
    container(
        row! {
            icon(MaterialSymbol::WARNING)
                .extent(Length::px(GLYPH))
                .color(palette().warning);
            text(tr!("ade.tabs.unsaved", name = &named(shown.path(), root)))
                .caption()
                .grow(1.0);
            answer(tr!("ade.tabs.save"), Answer::Save, false);
            answer(tr!("ade.tabs.discard"), Answer::Discard, true);
            answer(tr!("ade.tabs.keep"), Answer::Keep, true);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center),
    )
    .padding(Inset::xy(CARD_INSET, 8.0))
    .fill_width()
}

fn heading(shown: &Shown, root: &Path, git: &Git) -> impl View {
    row! {
        if let Some(state) = git.changed().file(shown.path()) {
            chip(state);
        }
        mono(named(shown.path(), root), CODE_FONT_SIZE - 1.0)
            .color(palette().text_muted)
            .no_wrap()
            .grow(1.0);
        if shown.is_unsaved() {
            icon(MaterialSymbol::CIRCLE)
                .extent(Length::px(DOT))
                .color(palette().accent);
        }
        if shown.is_stale() {
            reload();
        }
        if shown.is_readable() {
            save(shown);
        }
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::xy(CARD_INSET, 4.0))
    .fill_width()
}

fn named(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn chip(state: State) -> impl View {
    let color = git::ink(state, palette());
    row! {
        icon(git::glyph(state)).extent(Length::px(GLYPH)).color(color);
        text(git::label(state)).caption().color(color).no_wrap();
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
}

fn reload() -> impl View {
    action(MaterialSymbol::REFRESH, tr!("ade.file.reload"))
        .secondary()
        .on_press(act(|project| project.viewer(Viewer::reload)))
}

fn save(shown: &Shown) -> impl View {
    action(MaterialSymbol::SAVE, tr!("ade.file.save"))
        .loading(shown.is_saving())
        .enabled(shown.is_unsaved())
        .on_press(act(|project| project.viewer(|viewer, _| viewer.save())))
}

fn action(symbol: MaterialSymbol, label: Label) -> Button {
    button(
        row! {
            icon(symbol).extent(Length::px(GLYPH));
            text(label).caption().no_wrap();
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
    .small()
}

fn body(shown: &Shown) -> Element {
    if !shown.is_readable() {
        return container(match shown.error_message() {
            Some(error_message) => {
                warned(MaterialSymbol::ERROR, error_message, palette().danger).any()
            }
            None => hint(tr!("ade.file.reading")).any(),
        })
        .align_y(Align::Center)
        .grow(1.0)
        .fill_width()
        .any();
    }

    column! {
        find_mark(FIND);
        if let Some(banner) = banner(shown) {
            banner;
            rule();
        }
        text_editor(shown.document())
            .language(Language::from_path(shown.path()))
            .family(Family::parse(font::JETBRAINS_MONO))
            .font_size(CODE_FONT_SIZE)
            .line_height((CODE_FONT_SIZE * LINE_HEIGHT_FACTOR).round())
            .edits(shown.edits())
            .place(shown.place())
            .search(shown.search())
            .wrap(false)
            .code(true)
            .autofocus(true)
            .on_change(act(|project| project.viewer(|viewer, _| viewer.typed())))
            .grow(1.0)
            .fill_width()
            .build()
            .keyed(("ade-editor", shown.path()));
        find_bar(shown.search())
            .anchored_to(FIND)
            .replaceable(true)
            .on_action(given(|project, action: Action| {
                project.viewer(|viewer, _| viewer.find(action));
            }));
    }
    .grow(1.0)
    .fill_width()
    .any()
}

fn banner(shown: &Shown) -> Option<Container> {
    if let Some(error_message) = shown.error_message() {
        return Some(warned(
            MaterialSymbol::ERROR,
            error_message,
            palette().danger,
        ));
    }
    shown.is_stale().then(|| {
        warned(
            MaterialSymbol::WARNING,
            tr!("ade.file.meanwhile"),
            palette().warning,
        )
    })
}

fn warned(symbol: MaterialSymbol, message: impl Into<Label>, color: Color) -> Container {
    container(notice(symbol, message, color))
        .padding(Inset::xy(CARD_INSET, 10.0))
        .fill_width()
}
