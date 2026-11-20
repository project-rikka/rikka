use browser_pages::Cause;
use browser_translators::{CodeMode, Display, Engine};
use rikka_app::prelude::*;
use rikka_languages::Language;
use rikka_types::Choice;
use rikka_widgets::{column, row};

use crate::preferences::Rule;
use crate::shell::Shell;
use crate::translate::tabs::{Progress, TabStatus};
use crate::widgets::floating;
use browser_model::TabId;
use browser_pages::{Internal, Section};

const WIDTH: f32 = 320.0;
const PICKER: f32 = 128.0;
const BELOW: f32 = 6.0;
const ARROW: f32 = 16.0;

pub(crate) fn popover(shell: &Shell, target: &'static str) -> Option<Element> {
    let opened = shell.address().translate_popover()?;
    let model = shell.model();
    let tab_entry = model.active(shell.id())?;
    let tab = tab_entry.id;
    if tab != opened {
        return None;
    }
    let address = tab_entry.address();
    drop(model);
    crate::translate::tool(shell, tab, &address)?;
    let status = shell.browser().translations().tabs().status(tab);
    let site = browser_pages::web_site(&address);
    let body = column! {
        languages(shell, tab, &status);
        action(tab, &status);
        choices(shell, tab);
        rules(shell, tab, status.detected.as_ref(), site);
        row! {
            spacer().grow(1.0);
            button(text(tr!("browser.translate.settings")))
                .ghost()
                .small()
                .on_press(|shell: &mut Shell| {
                    shell.close_translate_popover();
                    shell.go(
                        Internal::Settings(Some(Section::Translation)).address(),
                        Cause::Typed,
                    );
                });
        }
        .fill_width();
    }
    .gap(Space::NORMAL)
    .padding(Inset::all(Space::NORMAL))
    .width(WIDTH);
    let panel = container(body)
        .glass()
        .background(palette().surface.with_alpha(0.94))
        .border(1.0, palette().outline.with_alpha(0.5))
        .radius(Corner::Panel);
    Some(
        floating(panel, |shell: &mut Shell| shell.close_translate_popover())
            .keyed("rikka-browser-translate-popover")
            .anchored(Anchor::new(target, AnchorEdge::BelowEnd).offset(0.0, BELOW))
            .transition(pop()),
    )
}

fn languages(shell: &Shell, tab: TabId, status: &TabStatus) -> Element {
    let source = match &status.detected {
        Some(detected) => tr!(
            "browser.translate.detected",
            language = detected.native_name()
        ),
        None => tr!("browser.translate.auto"),
    };
    let preferences = shell.browser().preferences();
    let targets = crate::translate::targets(preferences.translate_engine());
    let target = preferences.translate_to();
    let target_at = targets.position(&target);
    let endonyms: Vec<&'static str> = targets.endonyms().collect();
    let major = targets.major;
    let languages = targets.languages;
    row! {
        text(source)
            .label()
            .muted()
            .no_wrap()
            .shrink(1.0)
            .min_width(0.0);
        icon(MaterialSymbol::ARROW_FORWARD)
            .extent(Length::px(ARROW))
            .color(palette().text_muted);
        combo(target_at)
            .options(endonyms)
            .more(major, tr!("browser.translate.more-languages"))
            .control_size(ControlSize::Small)
            .focusless()
            .on_select(move |shell: &mut Shell, at: usize| {
                if let Some(target) = languages.get(at) {
                    shell.translate_into(tab, *target);
                }
            })
            .width(PICKER);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

fn action(tab: TabId, status: &TabStatus) -> Element {
    let (caption, tint) = match &status.progress {
        Progress::Failed(failure) => (
            Some(tr!(
                "browser.translate.failed",
                reason = failure.label().as_str()
            )),
            palette().danger,
        ),
        _ if status.busy => (
            Some(tr!("browser.translate.translating")),
            palette().text_muted,
        ),
        Progress::Translating | Progress::Translated => (
            Some(tr!("browser.translate.translated")),
            palette().text_muted,
        ),
        Progress::Original => (None, palette().text_muted),
    };
    let (label, restores) = match &status.progress {
        Progress::Failed(_) => (tr!("browser.translate.retry"), false),
        Progress::Translating | Progress::Translated => {
            (tr!("browser.translate.show-original"), true)
        }
        Progress::Original => (tr!("browser.translate.translate"), false),
    };
    let press = move |shell: &mut Shell| match restores {
        true => shell.show_original(tab),
        false => shell.start_translation(tab),
    };
    let pressed = match restores {
        true => button(text(label)).secondary(),
        false => button(text(label)).primary(),
    };
    row! {
        container(caption.map(|caption| text(caption).caption().color(tint)))
            .grow(1.0)
            .min_width(0.0);
        pressed.small().focusless().on_press(press);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

fn choices(shell: &Shell, tab: TabId) -> Element {
    let preferences = shell.browser().preferences();
    let display = preferences.translate_display();
    let code = preferences.translate_code();
    let engine = preferences.translate_engine();
    column! {
        segmented(Display::ALL.iter().position(|candidate| *candidate == display).unwrap_or(0))
            .segments(Display::ALL.iter().map(|display| crate::translate::display_name(*display)))
            .control_size(ControlSize::Small)
            .focusless()
            .on_select(move |shell: &mut Shell, at: usize| {
                if let Some(display) = Display::ALL.get(at) {
                    shell.translate_display(tab, *display);
                }
            })
            .fill_width();
        segmented(CodeMode::ALL.iter().position(|candidate| *candidate == code).unwrap_or(0))
            .segments(CodeMode::ALL.iter().map(|code| crate::translate::code_name(*code)))
            .control_size(ControlSize::Small)
            .focusless()
            .on_select(move |shell: &mut Shell, at: usize| {
                if let Some(code) = CodeMode::ALL.get(at) {
                    shell.translate_code(tab, *code);
                }
            })
            .fill_width();
        row! {
            text(tr!("browser.translate.service"))
                .label()
                .muted()
                .no_wrap()
                .grow(1.0)
                .min_width(0.0);
            combo(Engine::all().position(|candidate| candidate == engine))
                .options(Engine::all().map(Engine::label))
                .control_size(ControlSize::Small)
                .focusless()
                .on_select(move |shell: &mut Shell, at: usize| {
                    if let Some(engine) = Engine::all().nth(at) {
                        shell.translate_with(tab, engine);
                    }
                })
                .width(PICKER);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
    }
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

fn rules(shell: &Shell, tab: TabId, language: Option<&Language>, site: Option<String>) -> Element {
    let rules = shell.browser().preferences().translation_rules();
    let mut rows = Vec::new();
    if let Some(language) = language {
        let named = language.native_name();
        rows.push(rule_row(
            tab,
            Rule::AlwaysLanguage,
            language.tag(),
            tr!("browser.translate.always-language", language = named),
            rules.names_language(Rule::AlwaysLanguage, language),
        ));
        rows.push(rule_row(
            tab,
            Rule::NeverLanguage,
            language.tag(),
            tr!("browser.translate.never-language", language = named),
            rules.names_language(Rule::NeverLanguage, language),
        ));
    }
    if let Some(site) = site {
        let on = rules.names_site(Rule::NeverSite, &site);
        rows.push(rule_row(
            tab,
            Rule::NeverSite,
            &site,
            tr!("browser.translate.never-site"),
            on,
        ));
    }
    column_of(rows).gap(Space::TIGHT).fill_width().any()
}

fn rule_row(tab: TabId, rule: Rule, value: &str, label: Label, on: bool) -> Element {
    let value = value.to_owned();
    row! {
        text(label).label().grow(1.0).min_width(0.0);
        switch(on)
            .control_size(ControlSize::Small)
            .on_toggle(move |shell: &mut Shell, on: bool| {
                shell.translation_rule(tab, rule, &value, on);
            });
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

impl Shell {
    pub(crate) fn open_translate_popover(&mut self) {
        let tab = self.active_tab();
        self.address_mut().show_translate_popover(tab);
    }

    pub(crate) fn close_translate_popover(&mut self) {
        self.address_mut().show_translate_popover(None);
    }
}
