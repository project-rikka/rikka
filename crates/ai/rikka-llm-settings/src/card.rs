use crate::prelude::*;
use rikka_llm_accounts::{Account, Backend, Prompt, SignIn};
use rikka_widgets::{column, row};

use crate::llm_settings::LlmSettings;
use crate::rows::{self, indented};
use crate::signing::Signing;
use crate::wording;

pub(crate) struct Card<'a> {
    pub(crate) typed: Option<&'a TextDocument>,
    pub(crate) revealed: bool,
    pub(crate) in_use: bool,
    pub(crate) signing: Option<&'a Signing>,
    pub(crate) models: Option<Element>,
    pub(crate) usage: Option<Element>,
}

pub(crate) fn view(account: &Account, card: Card<'_>) -> Element {
    if let Some(signing) = card
        .signing
        .filter(|signing| Some(signing.backend()) == account.backend())
    {
        return waiting(account, signing).build();
    }

    let head = header(account, detail(account), card.in_use, actions(account));
    match details(account, card) {
        Some(under) => column! {
            head;
            indented(under);
        }
        .gap(Space::SNUG)
        .fill_width()
        .build(),
        None => head.build(),
    }
}

fn details(account: &Account, card: Card<'_>) -> Option<Element> {
    if card.typed.is_none() && card.usage.is_none() && card.models.is_none() {
        return None;
    }
    let revealed = card.revealed;
    Some(column! {
        if let (Some(typed), Some(backend)) = (card.typed, account.backend()) {
            rows::secret(
                typed,
                revealed,
                Handler::new(move |settings: &mut LlmSettings| settings.accounts.commit(backend, &mut settings.llm)),
                move |settings: &mut LlmSettings| {
                    settings.accounts.reveal(match revealed {
                        true => None,
                        false => Some(backend),
                    });
                },
            );
            if typed.is_empty() {
                environment(backend);
            }
        }
        for beside in [card.usage, card.models].into_iter().flatten() {
            spacer().height(4.0);
            beside;
        }
    }
    .gap(Space::TIGHT)
    .fill_width()
    .build())
}

fn actions(account: &Account) -> Vec<Element> {
    let Some(backend) = account.backend() else {
        return endpoint_actions(account);
    };
    let sign_in = || {
        button(tr!("llm-settings.sign-in"))
            .secondary()
            .small()
            .on_press(move |settings: &mut LlmSettings| settings.accounts.begin(backend))
            .build()
    };
    let leave = |label: Label| {
        button(label)
            .ghost()
            .small()
            .on_press(move |settings: &mut LlmSettings| {
                settings.accounts.sign_out(backend, &mut settings.llm)
            })
            .build()
    };

    match (
        backend.is_subscription(),
        backend.sign_in().is_interactive(),
    ) {
        (true, _) if account.has_credential() => vec![leave(tr!("llm-settings.sign-out"))],
        (true, _) => vec![sign_in()],
        (false, true) => vec![sign_in(), leave(tr!("llm-settings.remove"))],
        (false, false) => vec![leave(tr!("llm-settings.remove"))],
    }
}

fn endpoint_actions(account: &Account) -> Vec<Element> {
    let id = account.id();
    vec![
        button(tr!("llm-settings.endpoint.edit"))
            .secondary()
            .small()
            .on_press(move |settings: &mut LlmSettings| {
                settings.accounts.edit_endpoint(id, &settings.llm)
            })
            .build(),
        button(tr!("llm-settings.remove"))
            .ghost()
            .small()
            .on_press(move |settings: &mut LlmSettings| {
                settings.accounts.remove_endpoint(id, &mut settings.llm)
            })
            .build(),
    ]
}

fn environment(backend: Backend) -> Option<impl View> {
    let variable = backend.sign_in().variable()?;
    Some(
        text(tr!("llm-settings.from-environment", variable = variable))
            .caption()
            .muted(),
    )
}

fn waiting(account: &Account, signing: &Signing) -> impl View {
    column! {
        header(
            account,
            Some(rows::asking(tr!("llm-settings.waiting")).build()),
            false,
            Vec::new(),
        );
        signing.prompt().map(|prompt| {
            indented(match prompt {
                Prompt::Browser { url } => address(url, tr!("llm-settings.browser-hint")).build(),
                Prompt::Code { url, code } => device_code(url, code).build(),
            })
        });
        row! {
            spacer().grow(1.0);
            button(tr!("llm-settings.stop"))
                .ghost()
                .small()
                .on_press(|settings: &mut LlmSettings| settings.accounts.stop());
        }
        .align_items(Align::Center);
    }
    .gap(Space::NORMAL)
    .fill_width()
}

fn address(url: &str, hint: Label) -> impl View {
    column! {
        text(hint).caption().muted();
        row! {
            text(url.to_owned())
                .caption()
                .family(Family::parse(font::JETBRAINS_MONO))
                .grow(1.0);
            copy(url);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center);
    }
    .gap(Space::TIGHT)
    .fill_width()
}

fn device_code(url: &str, code: &str) -> impl View {
    column! {
        text(tr!("llm-settings.code-hint", url = url)).caption().muted();
        row! {
            text(code.to_owned())
                .title()
                .family(Family::parse(font::JETBRAINS_MONO))
                .semibold()
                .grow(1.0);
            copy(code);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center);
    }
    .gap(Space::TIGHT)
    .fill_width()
}

fn copy(what: &str) -> impl View {
    let what = what.to_owned();
    button(
        row! {
            icon(MaterialSymbol::CONTENT_COPY).extent(Length::px(14.0));
            text(tr!("llm-settings.copy")).caption();
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
    .ghost()
    .small()
    .on_press(move || set_text_of(Selection::Clipboard, &what))
}

fn header(
    account: &Account,
    detail: Option<Element>,
    in_use: bool,
    actions: Vec<Element>,
) -> impl View {
    let mut beside = Vec::new();
    if in_use {
        beside.push(
            text(tr!("llm-settings.in-use"))
                .caption()
                .color(TextColor::Accent)
                .no_wrap()
                .build(),
        );
    }
    beside.extend(actions);

    rows::header(
        account.is_ready(),
        account.label().to_owned(),
        detail,
        beside,
    )
}

fn detail(account: &Account) -> Option<Element> {
    let detail = match account.endpoint() {
        Some(endpoint) => endpoint.base().to_owned(),
        None => signed_detail(account),
    };
    (!detail.is_empty()).then(|| text(detail).caption().muted().build())
}

fn signed_detail(account: &Account) -> String {
    let masked = wording::masked(account.stored_key());
    match account.backend().map(Backend::sign_in) {
        Some(SignIn::Key { .. }) => masked,
        _ if !masked.is_empty() => masked,
        _ if account.has_credential() => account.identity(),
        _ => account.backend().map(hint).unwrap_or_default(),
    }
}

fn hint(backend: Backend) -> String {
    rikka_i18n::translate(&format!("llm-settings.hint.{}", backend.id()))
        .map(|hint| hint.to_string())
        .unwrap_or_default()
}
