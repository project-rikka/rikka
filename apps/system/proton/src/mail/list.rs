use std::sync::Arc;

use proton_mailbox::Row;

use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::mail::Mail;
use crate::{pane, wording};

const ROW: f32 = 74.0;

pub(crate) fn toolbar(mail: &Mail) -> impl View {
    pane::toolbar(
        row! {
            text_field(mail.search())
                .placeholder(tr!("proton.mail.search"))
                .claims_focus(mail.seeking())
                .on_blur(Mail::sought)
                .control_size(ControlSize::Small)
                .on_change(|mail: &mut Mail| mail.look())
                .grow(1.0);
            button(icon(MaterialSymbol::MARK_EMAIL_UNREAD).extent(Length::px(15.0)))
                .ghost()
                .small()
                .icon_only()
                .on_press(|mail: &mut Mail| {
                    let unread = !mail.is_unread_only();
                    mail.only_unread(unread);
                });
            button(icon(MaterialSymbol::REFRESH).extent(Length::px(15.0)))
                .ghost()
                .small()
                .icon_only()
                .loading(mail.is_busy())
                .on_press(|mail: &mut Mail| mail.refresh());
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
}

pub(crate) fn view(mail: &Mail) -> Element {
    let rows = Arc::clone(mail.mailbox().rows());
    let recipients = mail.mailbox().kind().shows_recipients();

    if rows.is_empty() {
        return match mail.is_busy() {
            true => pane::working(tr!("proton.mail.reading-list")).build(),
            false => pane::nothing(MaterialSymbol::INBOX, tr!("proton.mail.no-messages")).build(),
        };
    }

    let selected = mail.selected();
    let count = rows.len();

    column! {
        lazy_column(count, move |slot| {
            let row = &rows[slot.index];
            cell(row, selected == Some(slot.index), recipients)
                .fill_width()
                .height(ROW)
        })
        .uniform(ROW)
        .selected(selected)
        .place(mail.place())
        .shows(mail.mailbox().shown().as_str())
        .on_select(|mail: &mut Mail, at: usize| mail.select(at))
        .on_visible(|mail: &mut Mail, shown: std::ops::Range<usize>| {
            if shown.end + 8 >= mail.mailbox().rows().len() {
                mail.more();
            } else {
                skip_view();
            }
        })
        .grow(1.0)
        .fill_width();
        if mail.is_busy() {
            container(
                row! {
                    spinner().extent(Length::px(13.0));
                    text(tr!("proton.mail.reading-list")).caption().muted();
                }
                .gap(Space::SNUG)
                .align_items(Align::Center),
            )
            .padding(Space::SNUG)
            .center()
            .fill_width();
        }
    }
    .fill()
    .build()
}

fn cell(row: &Row, chosen: bool, recipients: bool) -> Element {
    let ink = match row.unread {
        true => palette().text,
        false => palette().text_muted,
    };
    let party = match row.party().is_empty() {
        true => tr!("proton.mail.nobody").as_str().to_owned(),
        false => row.party(),
    };
    let subject = match row.named().is_empty() {
        true => tr!("proton.mail.no-subject").as_str().to_owned(),
        false => row.named().to_owned(),
    };

    container(
        column! {
            row! {
                if row.unread {
                    container(spacer())
                        .background(palette().accent)
                        .radius(Corner::Detail)
                        .size(7.0, 7.0);
                }
                text(party)
                    .label()
                    .color(ink)
                    .weight(match row.unread {
                        true => Weight::SEMI_BOLD,
                        false => Weight::NORMAL,
                    })
                    .no_wrap()
                    .grow(1.0);
                if row.count > 1 {
                    pane::badge(row.count.to_string(), palette().text_muted);
                }
                text(wording::moment(row.time)).caption().muted().no_wrap();
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
            row! {
                text(subject).caption().color(ink).no_wrap().grow(1.0);
                if row.starred {
                    icon(MaterialSymbol::STAR)
                        .extent(Length::px(13.0))
                        .color(palette().warning);
                }
                if row.attachments {
                    icon(MaterialSymbol::ATTACH_FILE)
                        .extent(Length::px(13.0))
                        .color(palette().text_muted);
                }
                if row.draft && !recipients {
                    pane::badge(tr!("proton.mail.draft"), palette().accent);
                }
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .fill_width();
        }
        .gap(Space::TIGHT)
        .fill_width(),
    )
    .background(match chosen {
        true => palette().selection,
        false => Color::TRANSPARENT,
    })
    .padding(Inset::xy(12.0, 10.0))
    .fill_width()
    .build()
}
