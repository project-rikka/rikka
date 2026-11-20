use browser_engine_settings::EngineSettings;
use rikka_app::prelude::*;
use rikka_languages::Language;
use rikka_widgets::{column, row};

use crate::sections::engine::{CHOICE, change};
use crate::shell::Shell;

const GLYPH: f32 = 16.0;

type Listed = fn(&mut EngineSettings) -> &mut Vec<String>;

fn shown(code: &str) -> String {
    Language::lookup(code).map_or_else(
        || code.to_owned(),
        |language| language.native_name().to_owned(),
    )
}

fn edit(shell: &Shell, listed: Listed, edit: impl FnOnce(&mut Vec<String>)) {
    change(shell, |settings| edit(listed(settings)));
}

fn tool(symbol: MaterialSymbol) -> Button {
    button(icon(symbol).extent(Length::px(GLYPH)))
        .ghost()
        .icon_only()
        .small()
}

pub(crate) fn list(chosen: &[String], empty: Label, listed: Listed) -> Element {
    let rows: Vec<Element> = chosen
        .iter()
        .enumerate()
        .map(|(at, code)| {
            let removed = code.clone();
            row! {
                text(shown(code)).label().no_wrap().grow(1.0).min_width(0.0);
                tool(MaterialSymbol::ARROW_UPWARD)
                    .enabled(at > 0)
                    .on_press(move |shell: &mut Shell| {
                        edit(shell, listed, |codes| codes.swap(at - 1, at));
                    })
                    .tooltip(tr!("browser.languages.raise"));
                tool(MaterialSymbol::CLOSE)
                    .on_press(move |shell: &mut Shell| {
                        edit(shell, listed, |codes| codes.retain(|code| *code != removed));
                    })
                    .tooltip(tr!("browser.languages.remove"));
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width()
            .build()
            .keyed(code.clone())
        })
        .collect();
    let offered: Vec<Language> = Language::all()
        .filter(|language| !chosen.iter().any(|code| code == language.tag()))
        .collect();
    let names: Vec<&'static str> = offered
        .iter()
        .map(|language| language.native_name())
        .collect();
    let listed_rows = match rows.is_empty() {
        true => text(empty).muted().any(),
        false => column_of(rows).gap(Space::TIGHT).fill_width().any(),
    };
    column! {
        listed_rows;
        combo(0)
            .option(tr!("browser.languages.add"))
            .options(names)
            .on_select(move |shell: &mut Shell, at: usize| {
                if let Some(language) = at.checked_sub(1).and_then(|at| offered.get(at)) {
                    let code = language.tag().to_owned();
                    edit(shell, listed, |codes| codes.push(code));
                }
            })
            .width(CHOICE);
    }
    .gap(Space::NORMAL)
    .fill_width()
    .any()
}
