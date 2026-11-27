use std::ops::Range;
use std::sync::Arc;

use google_mail::{Error, Summary};
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use rikka_app::prelude::*;
use rikka_ui::hash_of;
use rikka_widgets::{column, row};

use crate::mail::sidebar::mailbox_title;
use crate::mail::{Folder, Listing, MailState, composer, reader, sidebar};

const LIST: f32 = 400.0;
const ROW: f32 = 84.0;
const STAR: Color = Color::rgb(0.98, 0.74, 0.02);

pub(crate) fn view(mail: &MailState) -> Element {
    let mut layers = vec![
        row! {
            sidebar::view(mail);
            rule_upright();
            threads(mail);
            rule_upright();
            container(reader::view(mail)).grow(1.0).min_width(0.0).fill_height();
        }
        .fill()
        .any(),
    ];
    if let Some(composing) = mail.composing() {
        layers.extend(composer::sheet(composing, mail.is_sending()));
    }
    column_of(layers).fill().any()
}

pub(crate) fn failure(error: &Error) -> String {
    match error {
        Error::Missing => tr!("google.mail.failure.missing").to_string(),
        Error::RateLimited => tr!("google.mail.failure.busy").to_string(),
        Error::NoRecipient => tr!("google.mail.compose.no-recipient").to_string(),
        Error::Address(address) => {
            tr!("google.mail.failure.address", address = address.as_str()).to_string()
        }
        other => other.to_string(),
    }
}

pub(crate) fn when(millis: i64) -> String {
    let Ok(moment) = Timestamp::from_millisecond(millis) else {
        return String::new();
    };
    let moment = moment.to_zoned(TimeZone::system());
    let today = Zoned::now();
    let format = if moment.date() == today.date() {
        "%H:%M"
    } else if moment.year() == today.year() {
        "%m/%d"
    } else {
        "%Y/%m/%d"
    };
    moment.strftime(format).to_string()
}

fn threads(mail: &MailState) -> impl View {
    let title = match mail.folder() {
        Folder::Mailbox(mailbox) => mailbox_title(*mailbox),
        Folder::Label { name, .. } => Label::from(name.clone()),
        Folder::Search(text) => tr!("google.mail.search.results", query = text.as_str()),
    };
    column! {
        row! {
            text(title).label().semibold().no_wrap().grow(1.0).min_width(0.0);
            button(icon(MaterialSymbol::REFRESH).extent(Length::px(18.0)))
                .ghost()
                .icon_only()
                .on_press(MailState::refresh)
                .tooltip(tr!("google.mail.refresh"));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .padding(Inset::edges(Space::NORMAL, Space::NORMAL, Space::TIGHT, Space::ROOMY))
        .fill_width();
        container(
            text_field(mail.search())
                .placeholder(tr!("google.mail.search.field"))
                .claims_focus(mail.is_seeking())
                .on_submit(MailState::sought)
                .on_blur(MailState::seek_ended)
                .fill_width(),
        )
        .padding(Inset::xy(Space::NORMAL, Space::TIGHT))
        .fill_width();
        if let Some(notice) = mail.notice() {
            let line = banner(notice.text.clone());
            container(
                match notice.danger {
                    true => line,
                    false => line.symbol(MaterialSymbol::INFO).tint(palette().accent),
                }
                .on_dismiss(tr!("google.dismiss"), MailState::dismiss_notice),
            )
            .padding(Inset::xy(Space::NORMAL, Space::TIGHT))
            .fill_width();
        }
        listing(mail);
        if mail.is_growing() {
            container(spinner().extent(Length::px(16.0)))
                .center()
                .padding(Inset::all(Space::SNUG))
                .fill_width();
        }
    }
    .width(LIST)
    .fill_height()
}

fn listing(mail: &MailState) -> Element {
    let threads = Arc::clone(mail.threads());
    match (mail.listing(), threads.is_empty()) {
        (Listing::Loading, true) => {
            return container(spinner().extent(Length::px(24.0)))
                .center()
                .grow(1.0)
                .fill_width()
                .any();
        }
        (Listing::Failed(reason), _) => {
            return status_page(MaterialSymbol::CLOUD_OFF, reason.clone())
                .action(button(text(tr!("google.mail.retry"))).on_press(MailState::refresh))
                .grow(1.0)
                .any();
        }
        (_, true) => {
            return status_page(MaterialSymbol::INBOX, tr!("google.mail.empty"))
                .grow(1.0)
                .any();
        }
        _ => {}
    }
    let chosen = mail
        .selected()
        .and_then(|id| threads.iter().position(|thread| thread.id == id));
    let me = mail
        .account()
        .map(|session| session.profile.email.to_lowercase())
        .unwrap_or_default();
    let named = Arc::clone(&threads);
    lazy_column(threads.len(), move |slot| entry(&threads[slot.index], &me))
        .estimate(ROW)
        .selected(chosen)
        .lit(true)
        .named(move |index| hash_of(&named[index].id))
        .on_select(|mail: &mut MailState, index: usize| mail.select(index))
        .on_visible(|mail: &mut MailState, shown: Range<usize>| mail.neared(shown))
        .padding(Inset::xy(Space::SNUG, Space::TIGHT))
        .grow(1.0)
        .fill_width()
        .any()
}

fn entry(thread: &Summary, me: &str) -> Element {
    let unread = thread.is_unread();
    let starred = thread.is_starred();
    let id = thread.id.clone();
    let mut senders = thread
        .senders
        .iter()
        .map(|sender| match sender.email.to_lowercase() == me {
            true => tr!("google.mail.me").to_string(),
            false => sender.short().to_owned(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    if thread.messages > 1 {
        senders = format!("{senders}  {}", thread.messages);
    }
    let subject = match thread.subject.is_empty() {
        true => tr!("google.mail.no-subject").to_string(),
        false => thread.subject.clone(),
    };
    let sender_line = text(senders).label().no_wrap().grow(1.0).min_width(0.0);
    let subject_line = text(subject).no_wrap().grow(1.0).min_width(0.0);
    container(
        column! {
            row! {
                if unread {
                    container(spacer())
                        .size(8.0, 8.0)
                        .radius(4.0)
                        .background(palette().accent);
                }
                match unread {
                    true => sender_line.bold(),
                    false => sender_line,
                };
                if thread.has_attachment {
                    icon(MaterialSymbol::ATTACH_FILE)
                        .extent(Length::px(14.0))
                        .color(palette().text_muted);
                }
                text(when(thread.date_ms)).caption().muted().no_wrap();
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
            row! {
                match unread {
                    true => subject_line.semibold(),
                    false => subject_line,
                };
                button(match starred {
                    true => icon(MaterialSymbol::STAR).filled().color(STAR),
                    false => icon(MaterialSymbol::STAR).color(palette().text_muted),
                }
                .extent(Length::px(16.0)))
                .ghost()
                .small()
                .icon_only()
                .focusless()
                .on_press(move |mail: &mut MailState| mail.toggle_star(&id));
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
            text(thread.snippet.clone()).caption().muted().no_wrap();
        }
        .gap(Space::HAIR)
        .fill_width(),
    )
    .padding(Inset::xy(Space::NORMAL, Space::SNUG))
    .fill_width()
    .any()
}
