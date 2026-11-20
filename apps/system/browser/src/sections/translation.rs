use browser_translators::{CodeMode, Display, Engine};
use rikka_app::prelude::*;
use rikka_languages::Language;
use rikka_types::Choice;
use rikka_widgets::{column, row};

use crate::preferences::Rule;
use crate::shell::Shell;
use crate::translate::{code_name, display_name};

const CHOICE: f32 = 220.0;
const GLYPH: f32 = 16.0;

pub(crate) fn view(shell: &Shell) -> Element {
    let preferences = shell.browser().preferences();
    let engine = preferences.translate_engine();
    let targets = crate::translate::targets(engine);
    let target = preferences.translate_target();
    let target_at = target
        .as_ref()
        .and_then(|target| targets.position(target))
        .map_or(0, |at| at + 1);
    let endonyms: Vec<&'static str> = targets.endonyms().collect();
    let major = targets.major;
    let languages = targets.languages;
    let display = preferences.translate_display();
    let code = preferences.translate_code();
    let rules = preferences.translation_rules();
    let lists =
        Rule::ALL.map(|rule| section(rule_title(rule), rule_list(rule, rules.list(rule).to_vec())));
    let [always_languages, never_languages, always_sites, never_sites] = lists;
    page([
        section(
            tr!("browser.translate.title"),
            column! {
                field(
                    tr!("browser.translate.target"),
                    combo(target_at)
                        .option(tr!("browser.translate.target-system"))
                        .options(endonyms)
                        .more(1 + major, tr!("browser.translate.more-languages"))
                        .on_select(move |shell: &mut Shell, at: usize| {
                            let target = at.checked_sub(1).and_then(|at| languages.get(at));
                            shell.browser().preferences().set_translate_target(target);
                        })
                        .width(CHOICE),
                );
                field(
                    tr!("browser.translate.service"),
                    combo(Engine::all().position(|candidate| candidate == engine))
                        .options(Engine::all().map(Engine::label))
                        .on_select(|shell: &mut Shell, at: usize| {
                            if let Some(engine) = Engine::all().nth(at) {
                                shell.browser().preferences().set_translate_engine(engine);
                            }
                        })
                        .width(CHOICE),
                );
                field(
                    tr!("browser.translate.display"),
                    combo(Display::ALL.iter().position(|candidate| *candidate == display))
                        .options(Display::ALL.iter().map(|display| display_name(*display)))
                        .on_select(|shell: &mut Shell, at: usize| {
                            if let Some(display) = Display::ALL.get(at) {
                                shell.browser().preferences().set_translate_display(*display);
                            }
                        })
                        .width(CHOICE),
                );
                field(
                    tr!("browser.translate.code"),
                    combo(CodeMode::ALL.iter().position(|candidate| *candidate == code))
                        .options(CodeMode::ALL.iter().map(|code| code_name(*code)))
                        .on_select(|shell: &mut Shell, at: usize| {
                            if let Some(code) = CodeMode::ALL.get(at) {
                                shell.browser().preferences().set_translate_code(*code);
                            }
                        })
                        .width(CHOICE),
                );
                field(
                    tr!("browser.translate.protect"),
                    switch(preferences.translate_protect()).on_toggle(
                        |shell: &mut Shell, on: bool| {
                            shell.browser().preferences().set_translate_protect(on);
                        },
                    ),
                );
                field(
                    tr!("browser.translate.offer"),
                    switch(preferences.translate_offer()).on_toggle(
                        |shell: &mut Shell, on: bool| {
                            shell.browser().preferences().set_translate_offer(on);
                        },
                    ),
                );
            }
            .gap(Space::NORMAL)
            .fill_width(),
        ),
        always_languages,
        never_languages,
        always_sites,
        never_sites,
    ])
    .any()
}

fn rule_title(rule: Rule) -> Label {
    match rule {
        Rule::AlwaysLanguage => tr!("browser.translate.always-languages"),
        Rule::NeverLanguage => tr!("browser.translate.never-languages"),
        Rule::AlwaysSite => tr!("browser.translate.always-sites"),
        Rule::NeverSite => tr!("browser.translate.never-sites"),
    }
}

fn rule_list(rule: Rule, entries: Vec<String>) -> Element {
    if entries.is_empty() {
        return text(tr!("browser.translate.none")).muted().any();
    }
    let symbol = match rule {
        Rule::AlwaysLanguage | Rule::NeverLanguage => MaterialSymbol::TRANSLATE,
        Rule::AlwaysSite | Rule::NeverSite => MaterialSymbol::PUBLIC,
    };
    let rows = entries.into_iter().map(|entry| {
        let shown = match rule {
            Rule::AlwaysLanguage | Rule::NeverLanguage => Language::lookup(&entry).map_or_else(
                || entry.clone(),
                |language| language.native_name().to_owned(),
            ),
            Rule::AlwaysSite | Rule::NeverSite => entry.clone(),
        };
        let removed = entry.clone();
        row! {
            icon(symbol).extent(Length::px(GLYPH)).color(palette().text_muted);
            text(shown).label().no_wrap().grow(1.0).min_width(0.0);
            button(icon(MaterialSymbol::CLOSE).extent(Length::px(GLYPH)))
                .ghost()
                .icon_only()
                .small()
                .on_press(move |shell: &mut Shell| {
                    shell
                        .browser()
                        .preferences()
                        .set_translation_rule(rule, &removed, false);
                });
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width()
        .build()
        .keyed(entry)
    });
    column_of(rows).gap(Space::TIGHT).fill_width().any()
}
