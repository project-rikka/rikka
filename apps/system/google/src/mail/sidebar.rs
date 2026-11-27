use google_mail::{Label as MailLabel, Mailbox};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::mail::{Folder, MailState};

pub(super) const WIDTH: f32 = 224.0;

pub(super) fn view(mail: &MailState) -> impl View {
    let labels = mail.labels();
    let count = |id: &str, total: bool| {
        labels
            .iter()
            .find(|label| label.id == id)
            .map_or(0, |label| match total {
                true => label.threads_total,
                false => label.threads_unread,
            })
    };
    let mut rows: Vec<ListItem> = Vec::new();
    let mut targets: Vec<Option<Folder>> = Vec::new();
    for mailbox in Mailbox::ALL {
        let waiting = match mailbox {
            Mailbox::Drafts => count(mailbox.id(), true),
            _ if mailbox.counts_unread() => count(mailbox.id(), false),
            _ => 0,
        };
        rows.push(counted(
            list_item(mailbox_title(mailbox)).icon(mailbox_symbol(mailbox)),
            waiting,
        ));
        targets.push(Some(Folder::Mailbox(mailbox)));
    }
    let listed: Vec<&MailLabel> = labels.iter().filter(|label| label.is_listed()).collect();
    if !listed.is_empty() {
        rows.push(list_header(tr!("google.mail.labels")));
        targets.push(None);
    }
    for label in listed {
        rows.push(counted(
            list_item(label.leaf().to_owned())
                .icon(MaterialSymbol::LABEL)
                .depth(label.depth()),
            label.threads_unread,
        ));
        targets.push(Some(Folder::Label {
            id: label.id.clone(),
            name: label.name.clone(),
        }));
    }
    let current = targets
        .iter()
        .position(|target| target.as_ref() == Some(mail.folder()));
    column! {
        container(
            button(
                row! {
                    icon(MaterialSymbol::EDIT).extent(Length::px(18.0));
                    text(tr!("google.mail.compose.new"));
                }
                .gap(Space::SNUG)
                .align_items(Align::Center),
            )
            .primary()
            .large()
            .on_press(MailState::compose)
            .fill_width(),
        )
        .padding(Inset::all(Space::NORMAL))
        .fill_width();
        list(rows)
            .selected(current)
            .on_select(move |mail: &mut MailState, index: usize| {
                if let Some(Some(folder)) = targets.get(index) {
                    mail.go(folder.clone());
                }
            })
            .padding(Inset::xy(Space::SNUG, Space::NONE))
            .grow(1.0)
            .fill_width();
    }
    .width(WIDTH)
    .fill_height()
}

pub(super) fn mailbox_title(mailbox: Mailbox) -> Label {
    match mailbox {
        Mailbox::Inbox => tr!("google.mail.mailbox.inbox"),
        Mailbox::Starred => tr!("google.mail.mailbox.starred"),
        Mailbox::Important => tr!("google.mail.mailbox.important"),
        Mailbox::Sent => tr!("google.mail.mailbox.sent"),
        Mailbox::Drafts => tr!("google.mail.mailbox.drafts"),
        Mailbox::AllMail => tr!("google.mail.mailbox.all"),
        Mailbox::Spam => tr!("google.mail.mailbox.spam"),
        Mailbox::Trash => tr!("google.mail.mailbox.trash"),
    }
}

fn mailbox_symbol(mailbox: Mailbox) -> MaterialSymbol {
    match mailbox {
        Mailbox::Inbox => MaterialSymbol::INBOX,
        Mailbox::Starred => MaterialSymbol::STAR,
        Mailbox::Important => MaterialSymbol::LABEL_IMPORTANT,
        Mailbox::Sent => MaterialSymbol::SEND,
        Mailbox::Drafts => MaterialSymbol::DRAFT,
        Mailbox::AllMail => MaterialSymbol::STACKED_EMAIL,
        Mailbox::Spam => MaterialSymbol::REPORT,
        Mailbox::Trash => MaterialSymbol::DELETE,
    }
}

fn counted(row: ListItem, waiting: u32) -> ListItem {
    match waiting {
        0 => row,
        waiting => row.trailing(waiting.to_string()),
    }
}
