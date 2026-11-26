use browser_pages::web_site;
use browser_preferences::MemorySaver;
use rikka_app::prelude::*;
use rikka_types::Choice;
use rikka_widgets::{column, row};

use crate::shell::Shell;

const CHOICE: f32 = 220.0;
const SITE: f32 = 260.0;

pub(crate) fn view(shell: &Shell) -> Element {
    let preferences = shell.browser().preferences();
    let chosen = preferences.memory_saver();
    let never = preferences.never_discarded();
    let saver = field(
        tr!("browser.memory.saver"),
        combo(MemorySaver::ALL.iter().position(|saver| *saver == chosen))
            .options(MemorySaver::ALL.iter().copied().map(saver_name))
            .on_select(|shell: &mut Shell, at: usize| {
                if let Some(saver) = MemorySaver::ALL.get(at) {
                    shell.browser().preferences().set_memory_saver(*saver);
                    shell.discard_idle(false);
                }
            })
            .width(CHOICE),
    );
    let sites = never.iter().cloned().map(|site| {
        let removed = site.clone();
        row! {
            icon(MaterialSymbol::PUBLIC).extent(Length::px(16.0)).color(palette().text_muted);
            text(site.clone()).label().no_wrap().grow(1.0).min_width(0.0);
            button(icon(MaterialSymbol::CLOSE).extent(Length::px(16.0)))
                .ghost()
                .icon_only()
                .small()
                .on_press(move |shell: &mut Shell| forget(shell, &removed));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width()
        .build()
        .keyed(site)
    });
    page([
        section(tr!("browser.memory.title"), saver),
        section(
            tr!("browser.memory.never"),
            column! {
                column_of(sites).gap(Space::TIGHT).fill_width();
                adding();
            }
            .gap(Space::NORMAL)
            .fill_width(),
        ),
    ])
    .any()
}

fn adding() -> Element {
    local_state(TextDocument::new(""), |draft: TextDocument, _| {
        let submitted = draft.clone();
        let pressed = draft.clone();
        row! {
            text_field(&draft)
                .placeholder(tr!("browser.memory.site"))
                .control_size(ControlSize::Small)
                .on_submit(move |shell: &mut Shell| add_exemption(shell, &submitted))
                .width(SITE);
            button(text(tr!("browser.memory.add")))
                .secondary()
                .small()
                .on_press(move |shell: &mut Shell| add_exemption(shell, &pressed));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
    })
}

fn add_exemption(shell: &mut Shell, draft: &TextDocument) {
    let Some(site) = site_of(&draft.text()) else {
        return;
    };
    let preferences = shell.browser().preferences();
    let mut never = preferences.never_discarded();
    if !never.contains(&site) {
        never.push(site);
        never.sort();
        preferences.set_never_discarded(never);
    }
    draft.reset("");
}

fn forget(shell: &mut Shell, site: &str) {
    let preferences = shell.browser().preferences();
    let mut never = preferences.never_discarded();
    never.retain(|candidate| candidate != site);
    preferences.set_never_discarded(never);
}

pub(crate) fn site_of(typed: &str) -> Option<String> {
    let typed = typed.trim();
    if typed.is_empty() {
        return None;
    }
    let address = match typed.contains("://") {
        true => typed.to_owned(),
        false => format!("https://{typed}"),
    };
    web_site(&address)
}

fn saver_name(saver: MemorySaver) -> Label {
    match saver {
        MemorySaver::Off => tr!("browser.memory.off"),
        MemorySaver::Quarter => tr!("browser.memory.quarter"),
        MemorySaver::Hour => tr!("browser.memory.hour"),
        MemorySaver::FourHours => tr!("browser.memory.four-hours"),
    }
}
