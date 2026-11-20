use std::sync::Arc;

use proton_mailbox::{Body, OpenedMessage, OpenedThread, Verification};
use rikka_app::prelude::*;
use rikka_markdown::markdown;
use rikka_proton_mail::SystemLabel;
use rikka_widgets::{column, row};

use crate::mail::{Mail, folders};
use crate::{pane, wording};

pub(crate) fn view(mail: &Mail, reading: &OpenedThread) -> impl View {
    let subject = match reading.subject.trim().is_empty() {
        true => tr!("proton.mail.no-subject").as_str().to_owned(),
        false => reading.subject.clone(),
    };

    column! {
        actions(mail, reading);
        rule();
        scrollable(
            column! {
                text(subject).title().semibold().fill_width();
                column_of(
                    reading
                        .messages
                        .iter()
                        .map(|message| letter(message).build()),
                )
                .gap(Space::ROOMY)
                .fill_width();
            }
            .gap(Space::ROOMY)
            .padding(Space::ROOMY)
            .fill_width(),
        )
        .grow(1.0)
        .fill_width();
    }
    .fill()
}

fn actions(mail: &Mail, reading: &OpenedThread) -> impl View {
    let starred = mail
        .shown()
        .map_or_else(|| reading.is_starred(), |row| row.starred);
    let drafts = mail.mailbox().kind().holds_drafts();
    let moves = folders::moving(mail);
    let labels = folders::labels(mail);
    let applied: Vec<rikka_proton_mail::LabelId> = mail
        .shown()
        .map(|row| row.labels.clone())
        .unwrap_or_default();

    pane::toolbar(
        row! {
            if drafts {
                button(
                    row! {
                        icon(MaterialSymbol::EDIT).extent(Length::px(15.0));
                        text(tr!("proton.mail.edit-draft")).caption().no_wrap();
                    }
                    .gap(Space::TIGHT)
                    .align_items(Align::Center),
                )
                .small()
                .on_press(|mail: &mut Mail| mail.edit_draft());
            } else {
                button(icon(MaterialSymbol::REPLY).extent(Length::px(15.0)))
                    .ghost()
                    .small()
                    .icon_only()
                    .on_press(|mail: &mut Mail| mail.answer(false));
                button(icon(MaterialSymbol::REPLY_ALL).extent(Length::px(15.0)))
                    .ghost()
                    .small()
                    .icon_only()
                    .on_press(|mail: &mut Mail| mail.answer(true));
                button(icon(MaterialSymbol::FORWARD).extent(Length::px(15.0)))
                    .ghost()
                    .small()
                    .icon_only()
                    .on_press(|mail: &mut Mail| mail.pass_on());
            }
            spacer().width(Space::SNUG.px());
            button(
                icon(match starred {
                    true => MaterialSymbol::STAR,
                    false => MaterialSymbol::STAR_OUTLINE,
                })
                .extent(Length::px(15.0))
                .color(match starred {
                    true => palette().warning,
                    false => palette().text_muted,
                }),
            )
            .ghost()
            .small()
            .icon_only()
            .on_press(move |mail: &mut Mail| mail.star(!starred));
            button(icon(MaterialSymbol::MARK_EMAIL_UNREAD).extent(Length::px(15.0)))
                .ghost()
                .small()
                .icon_only()
                .on_press(|mail: &mut Mail| mail.mark(false));
            button(icon(MaterialSymbol::ARCHIVE).extent(Length::px(15.0)))
                .ghost()
                .small()
                .icon_only()
                .on_press(|mail: &mut Mail| mail.move_to(SystemLabel::Archive.id()));
            button(icon(MaterialSymbol::DELETE).extent(Length::px(15.0)))
                .ghost()
                .small()
                .icon_only()
                .on_press(|mail: &mut Mail| mail.throw());
            if mail.mailbox().kind().deletes() {
                button(icon(MaterialSymbol::DELETE_FOREVER).extent(Length::px(15.0)))
                    .ghost()
                    .small()
                    .icon_only()
                    .on_press(|mail: &mut Mail| mail.erase());
            }
            spacer().grow(1.0);
            if !labels.is_empty() {
                combo(None)
                    .options(labels.iter().map(|(id, name)| match applied.contains(id) {
                        true => tr!("proton.mail.label-on", name = name.as_str()),
                        false => name.clone(),
                    }))
                    .placeholder(tr!("proton.mail.label"))
                    .control_size(ControlSize::Small)
                    .width(152.0)
                    .on_select(move |mail: &mut Mail, at: usize| {
                        if let Some((label, _)) = labels.get(at) {
                            mail.toggle_label(label.clone());
                        }
                    });
            }
            combo(None)
                .options(moves.iter().map(|(_, name, _)| name.clone()))
                .placeholder(tr!("proton.mail.move-to"))
                .control_size(ControlSize::Small)
                .width(168.0)
                .on_select(move |mail: &mut Mail, at: usize| {
                    if let Some((label, _, _)) = moves.get(at) {
                        mail.move_to(label.clone());
                    }
                });
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width(),
    )
}

fn letter(message: &OpenedMessage) -> impl View {
    let key = message.id.as_str().to_owned();

    card(
        column! {
            row! {
                pane::avatar(&wording::initial(message.sender.shown()), palette().accent);
                column! {
                    row! {
                        text(message.sender.shown().to_owned()).label().semibold().no_wrap();
                        text(message.sender.address.clone()).caption().muted().no_wrap();
                    }
                    .gap(Space::SNUG)
                    .align_items(Align::Center);
                    text(to(message)).caption().muted().no_wrap();
                }
                .gap(Space::HAIR)
                .grow(1.0);
                column! {
                    text(wording::whole(message.time)).caption().muted().no_wrap();
                    signature(message.signature);
                }
                .gap(Space::HAIR)
                .align_items(Align::End);
            }
            .gap(Space::NORMAL)
            .align_items(Align::Start)
            .fill_width();
            rule();
            body(&message.words, &key);
            files(message);
        }
        .gap(Space::NORMAL)
        .fill_width(),
    )
}

fn body(words: &Body, key: &str) -> Element {
    match words {
        Body::Markdown(rich) => markdown(Arc::clone(rich.source()))
            .on_link(|mail: &mut Mail, target: String| mail.follow(&target))
            .selectable(key.to_owned())
            .fill_width()
            .build(),
        Body::Plain(plain) => {
            let spans: Vec<TextSpan> = plain
                .links()
                .iter()
                .map(|range| {
                    TextSpan::new(range.clone())
                        .color(palette().accent)
                        .underline(true)
                })
                .collect();
            let linked = plain.clone();
            rich_text(Arc::clone(plain.text()))
                .spans(spans)
                .links(Arc::clone(plain.links()))
                .on_link(move |mail: &mut Mail, index: usize| {
                    if let Some(target) = linked.link(index) {
                        mail.follow(target);
                    }
                })
                .selectable(key.to_owned())
                .fill_width()
                .build()
        }
    }
}

fn files(message: &OpenedMessage) -> Option<Element> {
    let files = message.shown_files();
    if files.is_empty() {
        return None;
    }
    Some(
        column_of(files.into_iter().map(|file| {
            let attachment = file.clone();
            row! {
                icon(MaterialSymbol::ATTACH_FILE)
                    .extent(Length::px(15.0))
                    .color(palette().text_muted);
                text(file.name.clone()).caption().no_wrap().grow(1.0);
                text(wording::size(file.size)).caption().muted().no_wrap();
                button(icon(MaterialSymbol::DOWNLOAD).extent(Length::px(14.0)))
                    .ghost()
                    .small()
                    .icon_only()
                    .on_press(move |mail: &mut Mail| mail.save_file(attachment.clone()));
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width()
            .build()
        }))
        .gap(Space::TIGHT)
        .fill_width()
        .build(),
    )
}

fn signature(verification: Verification) -> impl View {
    let (symbol, tint, label) = match verification {
        Verification::Verified => (
            MaterialSymbol::LOCK,
            palette().success,
            tr!("proton.mail.verified"),
        ),
        Verification::Unsigned => (
            MaterialSymbol::LOCK_OPEN,
            palette().text_muted,
            tr!("proton.mail.unsigned"),
        ),
        Verification::Unknown => (
            MaterialSymbol::SHIELD_QUESTION,
            palette().text_muted,
            tr!("proton.mail.unknown-key"),
        ),
        Verification::Broken => (
            MaterialSymbol::REPORT,
            palette().danger,
            tr!("proton.mail.broken-signature"),
        ),
    };

    row! {
        icon(symbol).extent(Length::px(12.0)).color(tint);
        text(label).caption().color(tint).no_wrap();
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
}

fn to(message: &OpenedMessage) -> String {
    let named: Vec<&str> = message
        .to
        .iter()
        .chain(&message.cc)
        .map(|person| person.shown())
        .collect();

    match named.is_empty() {
        true => String::new(),
        false => tr!("proton.mail.to", who = &named.join(", "))
            .as_str()
            .to_owned(),
    }
}
