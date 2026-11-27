use google_passwords::{Credential, Error, Lock, Status};
use rikka_app::prelude::*;
use rikka_format::format_timestamp;
use rikka_widgets::{column, row};

use crate::desk::STAGE;
use crate::google::Google;
use crate::passwords::{PasswordState, editor};

const SIDEBAR: f32 = 336.0;
const DETAIL: f32 = 620.0;
const BADGE: f32 = 48.0;
const PASSPHRASE: f32 = 300.0;
const MASK: &str = "••••••••••••";
const DELETING: &str = "google-passwords-deleting";
const MILLIS: i64 = 1000;

pub(crate) fn view(passwords: &PasswordState) -> Element {
    let Some(status) = passwords.status() else {
        return container(spinner().extent(Length::px(24.0)))
            .center()
            .fill()
            .any();
    };
    let content = match status.lock {
        Lock::NeedsPassphrase => passphrase_page(passwords),
        Lock::NeedsVaultKeys => verification_page(),
        Lock::Open | Lock::AwaitingKeystoreKeys => catalog(passwords, status),
    };
    let mut layers = vec![content];
    if let Some(editing) = passwords.editing() {
        layers.extend(editor::sheet(editing));
    }
    if let Some(credential) = passwords.deleting() {
        layers.extend(
            confirm_prompt(
                tr!("google.passwords.delete.title"),
                tr!(
                    "google.passwords.delete.message",
                    site = credential.site.name.as_str()
                ),
                tr!("google.passwords.delete.accept"),
                |passwords: &mut PasswordState, confirmed: bool| {
                    passwords.answer_delete(confirmed);
                },
            )
            .keyed(DELETING)
            .centered_on(STAGE)
            .with_scrim(),
        );
    }
    column_of(layers).fill().any()
}

pub(crate) fn refusal(error: &Error) -> String {
    match error {
        Error::Address => tr!("google.passwords.refusal.address").to_string(),
        Error::EmptyPassword => tr!("google.passwords.refusal.password").to_string(),
        other => other.to_string(),
    }
}

fn catalog(passwords: &PasswordState, status: Status) -> Element {
    row! {
        sidebar(passwords, status);
        rule_upright();
        detail(passwords, status);
    }
    .fill()
    .any()
}

fn sidebar(passwords: &PasswordState, status: Status) -> impl View {
    let shown = passwords.shown();
    let chosen = passwords
        .selected()
        .and_then(|chosen| shown.iter().position(|shown| shown.id == chosen.id));
    let ids: Vec<String> = shown
        .iter()
        .map(|credential| credential.id.clone())
        .collect();
    let rows: Vec<ListItem> = shown.iter().map(|credential| entry(credential)).collect();
    column! {
        row! {
            text_field(passwords.search())
                .placeholder(tr!("google.passwords.search"))
                .on_change(PasswordState::search_changed)
                .grow(1.0);
            button(icon(MaterialSymbol::ADD))
                .primary()
                .icon_only()
                .on_press(PasswordState::add)
                .tooltip(tr!("google.passwords.add"));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .padding(Inset::edges(Space::NORMAL, Space::NORMAL, Space::SNUG, Space::NORMAL))
        .fill_width();
        progress_line(passwords, status);
        if let Some(failure) = passwords.failure() {
            container(
                banner(failure.to_owned())
                    .on_dismiss(tr!("google.dismiss"), PasswordState::dismiss_failure),
            )
            .padding(Inset::xy(Space::NORMAL, Space::TIGHT))
            .fill_width();
        }
        if rows.is_empty() {
            status_page(
                MaterialSymbol::SEARCH_OFF,
                match passwords.total() {
                    0 => tr!("google.passwords.empty"),
                    _ => tr!("google.passwords.no-match"),
                },
            )
            .grow(1.0);
        } else {
            list(rows)
                .selected(chosen)
                .on_select(move |passwords: &mut PasswordState, index: usize| {
                    passwords.select(ids.get(index).cloned());
                })
                .padding(Inset::xy(Space::SNUG, Space::TIGHT))
                .grow(1.0)
                .fill_width();
        }
    }
    .width(SIDEBAR)
    .fill_height()
}

fn entry(credential: &Credential) -> ListItem {
    let secondary = match (&credential.federation, credential.username.is_empty()) {
        (Some(provider), _) => tr!("google.passwords.federated", provider = provider.as_str()),
        (None, true) => tr!("google.passwords.no-username"),
        (None, false) => Label::from(credential.username.clone()),
    };
    list_item(credential.site.name.clone())
        .secondary(secondary)
        .icon(site_symbol(credential))
        .keyed(credential.id.clone())
}

fn site_symbol(credential: &Credential) -> MaterialSymbol {
    match credential.site.is_app {
        true => MaterialSymbol::ANDROID,
        false => MaterialSymbol::LANGUAGE,
    }
}

fn progress_line(passwords: &PasswordState, status: Status) -> impl View {
    let (mark, line): (Element, Label) = if passwords.is_syncing() {
        (
            spinner().extent(Length::px(14.0)).any(),
            tr!("google.passwords.syncing"),
        )
    } else if status.unsynced > 0 {
        (
            icon(MaterialSymbol::CLOUD_UPLOAD)
                .extent(Length::px(16.0))
                .color(palette().warning)
                .any(),
            tr!("google.passwords.unsent", count = status.unsynced),
        )
    } else if status.undecryptable > 0 {
        (
            icon(MaterialSymbol::LOCK)
                .extent(Length::px(16.0))
                .color(palette().warning)
                .any(),
            tr!("google.passwords.unreadable", count = status.undecryptable),
        )
    } else {
        (
            icon(MaterialSymbol::CLOUD_DONE)
                .extent(Length::px(16.0))
                .color(palette().success)
                .any(),
            tr!("google.passwords.synced", count = passwords.total()),
        )
    };
    row! {
        mark;
        text(line).caption().muted().no_wrap().grow(1.0);
        button(icon(MaterialSymbol::SYNC).extent(Length::px(16.0)))
            .ghost()
            .small()
            .icon_only()
            .enabled(!passwords.is_syncing())
            .on_press(|passwords: &mut PasswordState| {
                passwords.sync(Some(google_passwords::Refresh::Trigger));
            })
            .tooltip(tr!("google.passwords.sync-now"));
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::ROOMY, Space::TIGHT))
    .fill_width()
}

fn detail(passwords: &PasswordState, status: Status) -> Element {
    let Some(credential) = passwords.selected() else {
        let page = match (passwords.total(), status.has_synced) {
            (0, true) => status_page(MaterialSymbol::KEY, tr!("google.passwords.none-saved"))
                .action(
                    button(text(tr!("google.passwords.add")))
                        .primary()
                        .on_press(PasswordState::add),
                ),
            (0, false) => status_page(
                MaterialSymbol::CLOUD_SYNC,
                tr!("google.passwords.first-sync"),
            ),
            _ => status_page(MaterialSymbol::KEY, tr!("google.passwords.pick")),
        };
        return page.grow(1.0).fill_height().any();
    };
    let revealed = passwords.is_revealed();
    let body = column! {
        heading(credential);
        card(column! {
            field(
                tr!("google.passwords.username"),
                row! {
                    text(credential.username.clone()).no_wrap();
                    button(icon(MaterialSymbol::CONTENT_COPY).extent(Length::px(16.0)))
                        .ghost()
                        .small()
                        .icon_only()
                        .enabled(!credential.username.is_empty())
                        .on_press(PasswordState::copy_username)
                        .tooltip(tr!("google.passwords.copy-username"));
                }
                .gap(Space::SNUG)
                .align_items(Align::Center),
            );
            rule();
            field(tr!("google.passwords.password"), secret(credential, revealed));
            if !credential.note.is_empty() {
                rule();
                column! {
                    text(tr!("google.passwords.note")).label();
                    text(credential.note.clone()).muted();
                }
                .gap(Space::TIGHT)
                .padding(Inset::xy(Space::ROOMY, Space::NORMAL))
                .fill_width();
            }
        });
        text(dates(credential)).caption().muted();
    }
    .gap(Space::ROOMY)
    .padding(Inset::all(Space::LOOSE))
    .max_width(DETAIL)
    .fill_width();
    scrollable(container(body).align_x(Align::Center).fill_width())
        .shows(credential.id.clone())
        .grow(1.0)
        .fill_height()
        .transition(fade())
        .keyed(credential.id.clone())
}

fn heading(credential: &Credential) -> impl View {
    let initial: String = credential
        .site
        .group
        .chars()
        .next()
        .map(|letter| letter.to_uppercase().collect())
        .unwrap_or_default();
    row! {
        container(text(initial).title().bold().color(palette().on_accent))
            .center()
            .size(BADGE, BADGE)
            .radius(BADGE / 2.0)
            .background(palette().accent);
        column! {
            text(credential.site.name.clone()).title().semibold().no_wrap();
            if let Some(provider) = &credential.federation {
                text(tr!("google.passwords.federated", provider = provider.as_str()))
                    .caption()
                    .muted();
            }
        }
        .gap(Space::HAIR)
        .grow(1.0)
        .min_width(0.0);
        if !credential.site.address.is_empty() {
            button(icon(MaterialSymbol::OPEN_IN_NEW).extent(Length::px(18.0)))
                .ghost()
                .icon_only()
                .on_press(PasswordState::open_site)
                .tooltip(tr!("google.passwords.open-site"));
        }
        button(icon(MaterialSymbol::EDIT).extent(Length::px(18.0)))
            .ghost()
            .icon_only()
            .on_press(PasswordState::edit_selected)
            .tooltip(tr!("google.passwords.edit"));
        button(icon(MaterialSymbol::DELETE).extent(Length::px(18.0)))
            .ghost()
            .icon_only()
            .on_press(PasswordState::ask_delete)
            .tooltip(tr!("google.passwords.delete.accept"));
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .fill_width()
}

fn secret(credential: &Credential, revealed: bool) -> Element {
    if credential.password.is_empty() {
        return text(tr!("google.passwords.no-password")).muted().any();
    }
    let (shown, toggle, hint) = match revealed {
        true => (
            credential.password.clone(),
            MaterialSymbol::VISIBILITY_OFF,
            tr!("google.passwords.hide"),
        ),
        false => (
            MASK.to_owned(),
            MaterialSymbol::VISIBILITY,
            tr!("google.passwords.show"),
        ),
    };
    row! {
        text(shown).no_wrap();
        button(icon(toggle).extent(Length::px(16.0)))
            .ghost()
            .small()
            .icon_only()
            .on_press(PasswordState::toggle_revealed)
            .tooltip(hint);
        button(icon(MaterialSymbol::CONTENT_COPY).extent(Length::px(16.0)))
            .ghost()
            .small()
            .icon_only()
            .on_press(PasswordState::copy_password)
            .tooltip(tr!("google.passwords.copy-password"));
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .any()
}

fn dates(credential: &Credential) -> String {
    let moment = |millis: i64| format_timestamp(millis / MILLIS);
    [
        credential
            .created_ms
            .map(|at| tr!("google.passwords.created", time = moment(at).as_str())),
        credential
            .changed_ms
            .map(|at| tr!("google.passwords.changed", time = moment(at).as_str())),
        credential
            .used_ms
            .map(|at| tr!("google.passwords.used", time = moment(at).as_str())),
    ]
    .into_iter()
    .flatten()
    .map(|line| line.to_string())
    .collect::<Vec<_>>()
    .join("  ·  ")
}

fn passphrase_page(passwords: &PasswordState) -> Element {
    let refused = passwords.passphrase_refused();
    status_page(
        MaterialSymbol::LOCK,
        tr!("google.passwords.passphrase.needed"),
    )
    .action(
        column! {
            text_field(passwords.passphrase())
                .secret(true)
                .placeholder(tr!("google.passwords.passphrase.field"))
                .error(refused)
                .autofocus(true)
                .on_submit(PasswordState::unlock)
                .width(PASSPHRASE);
            if refused {
                text(tr!("google.passwords.passphrase.refused")).caption().danger();
            }
            button(text(tr!("google.passwords.passphrase.unlock")))
                .primary()
                .on_press(PasswordState::unlock);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center),
    )
    .any()
}

fn verification_page() -> Element {
    status_page(
        MaterialSymbol::VERIFIED_USER,
        tr!("google.passwords.verify.needed"),
    )
    .action(
        button(text(tr!("google.passwords.verify.start")))
            .primary()
            .on_press(Google::retrieve_vault_keys),
    )
    .any()
}
