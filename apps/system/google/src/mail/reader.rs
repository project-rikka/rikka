use std::sync::Arc;

use google_mail::{Address, Attachment, Mailbox, Message, Summary, Thread};
use rikka_app::prelude::*;
use rikka_format::format_bytes;
use rikka_markdown::markdown;
use rikka_widgets::{column, row};

use crate::avatar::initial;
use crate::mail::composer::{Answer, is_draft};
use crate::mail::view::when;
use crate::mail::{Folder, MailState, Reading};

const AVATAR: f32 = 36.0;
const READING: f32 = 860.0;

pub(super) fn view(mail: &MailState) -> Element {
    match mail.reading() {
        Reading::Nothing => status_page(MaterialSymbol::MAIL, tr!("google.mail.pick")).any(),
        Reading::Loading => container(spinner().extent(Length::px(24.0)))
            .center()
            .fill()
            .any(),
        Reading::Failed(reason) => status_page(MaterialSymbol::ERROR, reason.clone()).any(),
        Reading::Ready(thread) => opened(mail, thread),
    }
}

fn opened(mail: &MailState, thread: &Arc<Thread>) -> Element {
    let subject = thread
        .messages
        .first()
        .map(|message| message.subject.clone())
        .filter(|subject| !subject.is_empty())
        .unwrap_or_else(|| tr!("google.mail.no-subject").to_string());
    let last = thread.messages.len().saturating_sub(1);
    let blocked = mail.blocked_pictures();
    let body = column! {
        if blocked > 0 {
            banner(tr!("google.mail.pictures.blocked", count = blocked))
                .symbol(MaterialSymbol::IMAGE)
                .tint(palette().accent)
                .on_action(
                    tr!("google.mail.pictures.show"),
                    MailState::show_remote_pictures,
                );
        }
        for index in 0..thread.messages.len() {
            letter(mail, thread, index);
        }
        if let Some(message) = thread.messages.get(last).filter(|message| !is_draft(message)) {
            answers(thread, last, message);
        }
    }
    .gap(Space::NORMAL)
    .padding(Inset::all(Space::LOOSE))
    .max_width(READING)
    .fill_width();
    column! {
        row! {
            text(subject).title().semibold().grow(1.0).min_width(0.0);
            actions(mail);
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .padding(Inset::xy(Space::LOOSE, Space::NORMAL))
        .fill_width();
        rule();
        scrollable(container(body).align_x(Align::Center).fill_width())
            .shows(thread.id.clone())
            .grow(1.0)
            .fill_width();
    }
    .fill()
    .transition(fade())
    .keyed(thread.id.clone())
}

fn actions(mail: &MailState) -> impl View {
    let act = |symbol: MaterialSymbol| {
        button(icon(symbol).extent(Length::px(18.0)))
            .ghost()
            .icon_only()
    };
    let in_trash = *mail.folder() == Folder::Mailbox(Mailbox::Trash);
    let starred = mail.selected_summary().is_some_and(Summary::is_starred);
    let id = mail.selected().unwrap_or_default().to_owned();
    row! {
        act(MaterialSymbol::ARCHIVE)
            .enabled(!in_trash)
            .on_press(MailState::archive)
            .tooltip(tr!("google.mail.action.archive"));
        act(MaterialSymbol::REPORT)
            .enabled(!in_trash)
            .on_press(MailState::report_spam)
            .tooltip(tr!("google.mail.action.spam"));
        act(match in_trash {
            true => MaterialSymbol::RESTORE_FROM_TRASH,
            false => MaterialSymbol::DELETE,
        })
        .on_press(MailState::trash)
        .tooltip(match in_trash {
            true => tr!("google.mail.action.restore"),
            false => tr!("google.mail.action.trash"),
        });
        act(MaterialSymbol::MARK_EMAIL_UNREAD)
            .on_press(MailState::mark_unread)
            .tooltip(tr!("google.mail.action.unread"));
        button(match starred {
            true => icon(MaterialSymbol::STAR).filled().color(Color::rgb(0.98, 0.74, 0.02)),
            false => icon(MaterialSymbol::STAR),
        }
        .extent(Length::px(18.0)))
        .ghost()
        .icon_only()
        .on_press(move |mail: &mut MailState| mail.toggle_star(&id))
        .tooltip(match starred {
            true => tr!("google.mail.action.unstar"),
            false => tr!("google.mail.action.star"),
        });
        act(MaterialSymbol::CLOSE)
            .on_press(MailState::close_thread)
            .tooltip(tr!("google.mail.action.close"));
    }
    .gap(Space::HAIR)
    .align_items(Align::Center)
}

fn letter(mail: &MailState, thread: &Arc<Thread>, index: usize) -> Element {
    let message = &thread.messages[index];
    let expanded = mail.is_expanded(&message.id);
    let id = message.id.clone();
    let head = container(
        row! {
            initial(message.from.short(), AVATAR);
            column! {
                row! {
                    text(message.from.label().to_owned()).label().semibold().no_wrap();
                    if expanded && !message.from.name.is_empty() {
                        text(message.from.email.clone()).caption().muted().no_wrap();
                    }
                    if is_draft(message) {
                        text(tr!("google.mail.draft")).caption().danger().no_wrap();
                    }
                }
                .gap(Space::SNUG)
                .align_items(Align::Center);
                text(match expanded {
                    true => tr!("google.mail.to", recipients = listed(&message.to, &message.cc).as_str())
                        .to_string(),
                    false => message.snippet.clone(),
                })
                .caption()
                .muted()
                .no_wrap();
            }
            .gap(Space::HAIR)
            .grow(1.0)
            .min_width(0.0);
            text(when(message.date_ms)).caption().muted().no_wrap();
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center),
    )
    .on_press(move |mail: &mut MailState| mail.toggle_message(&id))
    .fill_width();
    let content = expanded.then(|| contents(mail, thread, index));
    container(
        column! {
            head;
            content;
        }
        .gap(Space::NORMAL)
        .fill_width(),
    )
    .padding(Inset::all(Space::ROOMY))
    .radius(Corner::Panel)
    .surface()
    .fill_width()
    .any()
}

fn contents(mail: &MailState, thread: &Arc<Thread>, index: usize) -> Element {
    let message = &thread.messages[index];
    let source = mail.rendered(&message.id).unwrap_or_else(|| Arc::from(""));
    let mut body = markdown(source)
        .on_link(|mail: &mut MailState, link: String| mail.open_link(&link))
        .selectable(message.id.clone());
    if let Some(folder) = mail.pictures_folder() {
        body = body.base(folder);
    }
    let files: Vec<Element> = message.files().map(enclosure).collect();
    let draft = Arc::clone(thread);
    column! {
        body.fill_width();
        if !files.is_empty() {
            row_of(files).gap(Space::SNUG).wrap_items().fill_width();
        }
        if is_draft(message) {
            button(text(tr!("google.mail.edit-draft")))
                .on_press(move |mail: &mut MailState| mail.edit_draft(&draft.messages[index]));
        }
    }
    .gap(Space::NORMAL)
    .fill_width()
    .any()
}

fn enclosure(attachment: &Attachment) -> Element {
    let (opened, saved) = (attachment.clone(), attachment.clone());
    container(
        row! {
            icon(MaterialSymbol::ATTACH_FILE).extent(Length::px(16.0));
            column! {
                text(attachment.name.clone()).label().no_wrap();
                text(format_bytes(attachment.size)).caption().muted();
            }
            .gap(Space::HAIR);
            button(icon(MaterialSymbol::DOWNLOAD).extent(Length::px(16.0)))
                .ghost()
                .small()
                .icon_only()
                .on_press(move |mail: &mut MailState| mail.save_attachment(&saved))
                .tooltip(tr!("google.mail.attachment.save"));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center),
    )
    .padding(Inset::xy(Space::NORMAL, Space::SNUG))
    .radius(Corner::Item)
    .background(palette().surface_variant)
    .on_press(move |mail: &mut MailState| mail.open_attachment(&opened))
    .tooltip(tr!("google.mail.attachment.open"))
}

fn answers(thread: &Arc<Thread>, index: usize, message: &Message) -> impl View {
    let answer = |symbol, label: Label, answer: Answer| {
        let thread = Arc::clone(thread);
        button(
            row! {
                icon(symbol).extent(Length::px(16.0));
                text(label);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center),
        )
        .on_press(move |mail: &mut MailState| mail.answer(&thread.messages[index], answer))
    };
    let several = message.to.len() + message.cc.len() > 1;
    row! {
        answer(MaterialSymbol::REPLY, tr!("google.mail.action.reply"), Answer::Reply);
        if several {
            answer(
                MaterialSymbol::REPLY_ALL,
                tr!("google.mail.action.reply-all"),
                Answer::ReplyAll,
            );
        }
        answer(
            MaterialSymbol::FORWARD,
            tr!("google.mail.action.forward"),
            Answer::Forward,
        );
    }
    .gap(Space::SNUG)
}

fn listed(to: &[Address], cc: &[Address]) -> String {
    to.iter()
        .chain(cc)
        .map(Address::short)
        .collect::<Vec<_>>()
        .join(", ")
}
