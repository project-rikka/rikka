use proton_mailbox::{Folder, Kind};
use rikka_app::prelude::*;
use rikka_proton_mail::LabelId;
use rikka_widgets::{column, row};

use crate::mail::Mail;
use crate::{pane, wording};

pub(crate) fn view(mail: &Mail) -> impl View {
    let shown = mail.mailbox().shown().clone();

    column! {
        container(
            button(
                row! {
                    icon(MaterialSymbol::EDIT_SQUARE).extent(Length::px(16.0));
                    text(tr!("proton.mail.write")).no_wrap();
                }
                .gap(Space::SNUG)
                .align_items(Align::Center),
            )
            .primary()
            .on_press(|mail: &mut Mail| mail.write())
            .fill_width(),
        )
        .padding(Inset::xy(12.0, 12.0))
        .fill_width();
        scrollable(
            column_of(
                mail.mailbox()
                    .folders()
                    .iter()
                    .map(|folder| leaf(folder, folder.id == shown).build()),
            )
            .gap(Space::HAIR)
            .padding(Inset::xy(8.0, 0.0))
            .fill_width(),
        )
        .grow(1.0)
        .fill_width();
    }
    .width(pane::SIDEBAR)
    .fill_height()
}

fn leaf(folder: &Folder, chosen: bool) -> impl View {
    let tint = match chosen {
        true => palette().accent,
        false => palette().text,
    };
    let id = folder.id.clone();
    let unread = folder.unread;

    container(
        row! {
            spacer().width(f32::from(u8::try_from(folder.depth).unwrap_or(0)) * 12.0);
            icon(symbol(folder.kind)).extent(Length::px(16.0)).color(tint);
            text(named(folder)).label().color(tint).no_wrap().grow(1.0);
            if unread > 0 {
                text(wording::count(usize::try_from(unread).unwrap_or_default()))
                    .caption()
                    .color(match chosen {
                        true => palette().accent,
                        false => palette().text_muted,
                    })
                    .no_wrap();
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .background(match chosen {
        true => palette().accent.with_alpha(0.12),
        false => Color::TRANSPARENT,
    })
    .radius(Corner::Item)
    .padding(Inset::xy(10.0, 7.0))
    .fill_width()
    .on_press(move |mail: &mut Mail| mail.show(id.clone()))
}

pub(crate) fn named(folder: &Folder) -> Label {
    match folder.kind {
        Kind::Inbox => tr!("proton.mail.folder.inbox"),
        Kind::Drafts => tr!("proton.mail.folder.drafts"),
        Kind::Scheduled => tr!("proton.mail.folder.scheduled"),
        Kind::Sent => tr!("proton.mail.folder.sent"),
        Kind::Starred => tr!("proton.mail.folder.starred"),
        Kind::Archive => tr!("proton.mail.folder.archive"),
        Kind::Spam => tr!("proton.mail.folder.spam"),
        Kind::Trash => tr!("proton.mail.folder.trash"),
        Kind::All => tr!("proton.mail.folder.all"),
        Kind::Folder | Kind::Label => Label::from(folder.name.as_str()),
    }
}

pub(crate) fn symbol(kind: Kind) -> MaterialSymbol {
    match kind {
        Kind::Inbox => MaterialSymbol::INBOX,
        Kind::Drafts => MaterialSymbol::DRAFT,
        Kind::Scheduled => MaterialSymbol::SCHEDULE,
        Kind::Sent => MaterialSymbol::SEND,
        Kind::Starred => MaterialSymbol::STAR,
        Kind::Archive => MaterialSymbol::ARCHIVE,
        Kind::Spam => MaterialSymbol::REPORT,
        Kind::Trash => MaterialSymbol::DELETE,
        Kind::All => MaterialSymbol::MAIL,
        Kind::Folder => MaterialSymbol::FOLDER,
        Kind::Label => MaterialSymbol::LABEL,
    }
}

#[must_use]
pub(crate) fn moving(mail: &Mail) -> Vec<(LabelId, Label, MaterialSymbol)> {
    mail.mailbox()
        .folders()
        .iter()
        .filter(|folder| {
            matches!(
                folder.kind,
                Kind::Inbox | Kind::Archive | Kind::Spam | Kind::Folder
            )
        })
        .map(|folder| (folder.id.clone(), named(folder), symbol(folder.kind)))
        .collect()
}

#[must_use]
pub(crate) fn labels(mail: &Mail) -> Vec<(LabelId, Label)> {
    mail.mailbox()
        .folders()
        .iter()
        .filter(|folder| folder.kind == Kind::Label)
        .map(|folder| (folder.id.clone(), Label::from(folder.name.as_str())))
        .collect()
}
