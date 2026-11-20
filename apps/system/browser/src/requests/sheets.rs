use rikka_app::prelude::*;
use rikka_format::truncate;
use rikka_web::{ButtonLabel, DateTimeMode};
use rikka_widgets::{color_picker, column, row};

use crate::requests::dialog::Dialog;
use crate::requests::{TabRequests, signing};
use crate::shell::Shell;
use crate::widgets::confirm_keys;
use browser_model::TabId;

const WIDTH: f32 = 420.0;
const TOP: f32 = 24.0;
const LONGEST: usize = 4000;
const CHOICES: usize = 8;

fn prepare(tab_requests: &mut TabRequests) {
    let (seed, checked) = match tab_requests.dialogs.current() {
        Some(Dialog::Text(prompt)) => (prompt.value.clone(), None),
        Some(Dialog::Color(prompt)) => (prompt.value.clone(), None),
        Some(Dialog::DateTime(prompt)) => (prompt.value.clone(), None),
        Some(dialog) => (None, dialog.check().map(|check| check.checked)),
        None => (None, None),
    };
    tab_requests.checked = checked.unwrap_or(false);
    tab_requests.answer.reset(seed.unwrap_or_default());
    tab_requests.answer.select_all();
}

impl Shell {
    pub(crate) fn dialog_arrived(&mut self, tab: TabId, dialog: Dialog) {
        let page_services = self.browser().page_services();
        let mut requests = page_services.requests_mut();
        let tab_requests = requests.edit(tab);
        if tab_requests.dialogs.push(dialog) {
            prepare(tab_requests);
        }
    }

    pub(crate) fn dialog_withdrawn(&mut self, tab: TabId, id: &rikka_web::PromptId) {
        let page_services = self.browser().page_services();
        let mut requests = page_services.requests_mut();
        let Some(tab_requests) = requests.edit_existing(tab) else {
            return;
        };
        let current = tab_requests.dialogs.current().map(Dialog::id) == Some(id);
        drop(tab_requests.dialogs.withdraw(id));
        if current {
            prepare(tab_requests);
        }
    }

    fn take_dialog(&mut self, tab: TabId) -> Option<(Dialog, String, Option<bool>)> {
        let page_services = self.browser().page_services();
        let mut requests = page_services.requests_mut();
        let tab_requests = requests.edit_existing(tab)?;
        let typed = tab_requests.answer.text();
        let dialog = tab_requests.dialogs.take()?;
        let checked = dialog.check().map(|_| tab_requests.checked);
        prepare(tab_requests);
        Some((dialog, typed, checked))
    }

    pub(crate) fn answer_dialog(&mut self, tab: TabId, accepted: bool) {
        let Some((dialog, typed, checked)) = self.take_dialog(tab) else {
            return;
        };
        dialog.answer(accepted, typed, checked);
    }

    pub(crate) fn pick_in_dialog(&mut self, tab: TabId, index: usize) {
        if let Some((dialog, _, _)) = self.take_dialog(tab) {
            dialog.pick(index);
        }
    }

    pub(crate) fn check_dialog(&mut self, tab: TabId, on: bool) {
        if let Some(requests) = self
            .browser()
            .page_services()
            .requests_mut()
            .edit_existing(tab)
        {
            requests.checked = on;
        }
    }

    fn color_moved(&mut self, tab: TabId, color: Color) {
        if let Some(tab_requests) = self.browser().page_services().requests_mut().quietly(tab) {
            tab_requests.answer.set_text(rgb_hex(color));
        }
        skip_view();
    }
}

fn rgb_hex(color: Color) -> String {
    let [r, g, b, _] = color.to_rgba8();
    format!("#{r:02x}{g:02x}{b:02x}")
}

pub(crate) fn sheet(tab_requests: &TabRequests, tab: TabId, site: &str) -> Option<Element> {
    if let Some(dialog) = tab_requests.dialogs.current() {
        return Some(scrimmed(dialog_card(tab_requests, dialog, tab, site)));
    }
    tab_requests
        .signing
        .as_ref()
        .map(|signing| scrimmed(signing::card(signing, tab)))
}

pub(crate) fn scrimmed(card: Element) -> Element {
    column_of(vec![scrim().build(), card])
        .align_items(Align::Center)
        .padding(Inset::edges(
            TOP,
            Space::NORMAL,
            Space::NORMAL,
            Space::NORMAL,
        ))
        .position(0.0, 0.0)
        .fill()
        .transition(fade())
}

pub(crate) fn framed(content: impl View) -> impl View {
    container(content)
        .padding(Inset::all(Space::WIDE))
        .background(palette().surface)
        .border(1.0, palette().outline.with_alpha(0.6))
        .radius(Corner::Panel)
        .width(WIDTH)
}

struct Shape {
    title: Label,
    message: Option<String>,
    accept: Label,
    cancel: Option<Label>,
}

fn site_message(
    site: &str,
    message: Option<&String>,
    accept: Label,
    cancel: Option<Label>,
) -> Shape {
    Shape {
        title: tr!("browser.dialog.says", site = site),
        message: message.map(|message| truncate(message, LONGEST).into_owned()),
        accept,
        cancel,
    }
}

fn button_label(label: &ButtonLabel) -> Label {
    match label {
        ButtonLabel::Ok => tr!("browser.dialog.ok"),
        ButtonLabel::Cancel => tr!("browser.dialog.cancel"),
        ButtonLabel::Yes => tr!("browser.dialog.yes"),
        ButtonLabel::No => tr!("browser.dialog.no"),
        ButtonLabel::Custom(label) => Label::from(label.clone()),
    }
}

fn shape(dialog: &Dialog, site: &str) -> Shape {
    let ok = || tr!("browser.dialog.ok");
    let cancel = || Some(tr!("browser.dialog.cancel"));
    match dialog {
        Dialog::Alert(prompt) => site_message(site, prompt.message.as_ref(), ok(), None),
        Dialog::Confirm(prompt) => {
            let positive = prompt
                .buttons
                .iter()
                .find(|button| button.kind == rikka_web::ButtonType::Positive)
                .map_or_else(ok, |button| button_label(&button.label));
            let other = prompt
                .buttons
                .iter()
                .find(|button| button.kind != rikka_web::ButtonType::Positive)
                .map(|button| button_label(&button.label));
            site_message(
                site,
                prompt.message.as_ref(),
                positive,
                other.or_else(cancel),
            )
        }
        Dialog::Text(prompt) => site_message(site, prompt.message.as_ref(), ok(), cancel()),
        Dialog::Leave(_) => Shape {
            title: tr!("browser.dialog.leave-title"),
            message: Some(tr!("browser.dialog.leave-message").to_string()),
            accept: tr!("browser.dialog.leave"),
            cancel: Some(tr!("browser.dialog.stay")),
        },
        Dialog::Repost(_) => Shape {
            title: tr!("browser.dialog.repost-title"),
            message: Some(tr!("browser.dialog.repost-message").to_string()),
            accept: tr!("browser.dialog.resend"),
            cancel: cancel(),
        },
        Dialog::Popup(prompt) => Shape {
            title: tr!("browser.dialog.popup-title", site = site),
            message: prompt.target.clone(),
            accept: tr!("browser.dialog.allow"),
            cancel: Some(tr!("browser.dialog.block")),
        },
        Dialog::Redirect(prompt) => Shape {
            title: tr!("browser.dialog.redirect-title", site = site),
            message: prompt.target.clone(),
            accept: tr!("browser.dialog.allow"),
            cancel: Some(tr!("browser.dialog.block")),
        },
        Dialog::FolderUpload(prompt) => Shape {
            title: tr!("browser.dialog.upload-title", folder = &prompt.folder),
            message: Some(tr!("browser.dialog.upload-message", site = site).to_string()),
            accept: tr!("browser.dialog.upload"),
            cancel: cancel(),
        },
        Dialog::Color(_) => Shape {
            title: tr!("browser.dialog.color-title"),
            message: None,
            accept: tr!("browser.dialog.choose"),
            cancel: cancel(),
        },
        Dialog::DateTime(prompt) => Shape {
            title: date_title(prompt.mode),
            message: Some(date_format(prompt.mode).to_owned()),
            accept: tr!("browser.dialog.choose"),
            cancel: cancel(),
        },
        Dialog::Login(_) | Dialog::Card(_) | Dialog::Address(_) => Shape {
            title: tr!("browser.dialog.fill-title"),
            message: None,
            accept: ok(),
            cancel: cancel(),
        },
    }
}

fn date_title(mode: DateTimeMode) -> Label {
    match mode {
        DateTimeMode::Date | DateTimeMode::Month | DateTimeMode::Week => {
            tr!("browser.dialog.date-title")
        }
        DateTimeMode::Time => tr!("browser.dialog.time-title"),
        DateTimeMode::DatetimeLocal => tr!("browser.dialog.date-time-title"),
    }
}

fn date_format(mode: DateTimeMode) -> &'static str {
    match mode {
        DateTimeMode::Date => "YYYY-MM-DD",
        DateTimeMode::Month => "YYYY-MM",
        DateTimeMode::Week => "YYYY-Www",
        DateTimeMode::Time => "HH:MM",
        DateTimeMode::DatetimeLocal => "YYYY-MM-DDTHH:MM",
    }
}

fn choices(dialog: &Dialog) -> Vec<String> {
    let labels: Vec<String> = match dialog {
        Dialog::Login(prompt) => prompt
            .options
            .iter()
            .map(|option| match option.value.username.is_empty() {
                true => option.value.origin.clone(),
                false => option.value.username.clone(),
            })
            .collect(),
        Dialog::Card(prompt) => prompt
            .options
            .iter()
            .map(|option| {
                let digits: String = option.value.number.chars().rev().take(4).collect();
                let last: String = digits.chars().rev().collect();
                format!("{} ••{last}", option.value.name)
            })
            .collect(),
        Dialog::Address(prompt) => prompt
            .options
            .iter()
            .map(|option| format!("{} · {}", option.value.name, option.value.street_address))
            .collect(),
        _ => Vec::new(),
    };
    labels.into_iter().take(CHOICES).collect()
}

fn field(tab_requests: &TabRequests, dialog: &Dialog, tab: TabId) -> Option<Element> {
    match dialog {
        Dialog::Text(_) | Dialog::DateTime(_) => Some(
            text_field(&tab_requests.answer)
                .claims_focus(true)
                .on_submit(move |shell: &mut Shell| shell.answer_dialog(tab, true))
                .fill_width()
                .any(),
        ),
        Dialog::Color(_) => {
            let color = Color::parse_hex(&tab_requests.answer.text()).unwrap_or(Color::BLACK);
            Some(
                color_picker(color)
                    .alpha(false)
                    .on_change(move |shell: &mut Shell, color: Color| shell.color_moved(tab, color))
                    .any(),
            )
        }
        Dialog::Login(_) | Dialog::Card(_) | Dialog::Address(_) => Some(
            column_of(
                choices(dialog)
                    .into_iter()
                    .enumerate()
                    .map(|(index, label)| {
                        button(text(label).no_wrap())
                            .ghost()
                            .fill_width()
                            .on_press(move |shell: &mut Shell| shell.pick_in_dialog(tab, index))
                            .any()
                    }),
            )
            .gap(Space::HAIR)
            .fill_width()
            .any(),
        ),
        _ => None,
    }
}

fn dialog_card(tab_requests: &TabRequests, dialog: &Dialog, tab: TabId, site: &str) -> Element {
    let Shape {
        title,
        message,
        accept,
        cancel,
    } = shape(dialog, site);
    let message = message.filter(|message| !message.is_empty());
    let check = dialog.check().map(|check| {
        checkbox(tab_requests.checked)
            .label(text(check.label.clone()).caption())
            .on_toggle(move |shell: &mut Shell, on: bool| shell.check_dialog(tab, on))
    });
    let buttons = row! {
        spacer().grow(1.0);
        cancel.map(|label| {
            button(text(label))
                .secondary()
                .on_press(move |shell: &mut Shell| shell.answer_dialog(tab, false))
        });
        button(text(accept))
            .primary()
            .on_press(move |shell: &mut Shell| shell.answer_dialog(tab, true));
    }
    .gap(Space::SNUG)
    .fill_width();
    let card = framed(
        column! {
            text(title).label().semibold();
            message.map(|message| text(message).fill_width());
            field(tab_requests, dialog, tab);
            check;
            buttons;
        }
        .gap(Space::NORMAL)
        .fill_width(),
    );
    confirm_keys(
        card,
        move |shell: &mut Shell| shell.answer_dialog(tab, true),
        move |shell: &mut Shell| shell.answer_dialog(tab, false),
    )
}
