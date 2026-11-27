use std::path::PathBuf;

use google_mail::{Address, Attachment, DRAFT, Enclosure, Message, Outgoing};
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse};
use rikka_format::{format_bytes, format_timestamp};
use rikka_widgets::{column, row};

use crate::desk::STAGE;
use crate::mail::{MailState, view};

const WIDTH: f32 = 680.0;
const BODY: std::ops::RangeInclusive<f32> = 220.0..=380.0;
const KEY: &str = "google-mail-composing";
const MILLIS: i64 = 1000;
const REPLY: &str = "Re: ";
const FORWARD: &str = "Fwd: ";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Delivery {
    Send,
    Keep,
    SetAside,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Answer {
    Reply,
    ReplyAll,
    Forward,
}

#[derive(Clone, Debug)]
pub(crate) struct Picked {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
}

pub(crate) struct Composing {
    pub(crate) to: TextDocument,
    pub(crate) cc: TextDocument,
    pub(crate) bcc: TextDocument,
    pub(crate) subject: TextDocument,
    pub(crate) body: TextDocument,
    pub(crate) copies: bool,
    pub(crate) files: Vec<Picked>,
    pub(crate) forwarded: Vec<Attachment>,
    pub(crate) thread: Option<String>,
    pub(crate) in_reply_to: Option<String>,
    pub(crate) references: Vec<String>,
    pub(crate) draft: Option<String>,
    pub(crate) refusal: Option<String>,
    pub(crate) key: u64,
}

impl Composing {
    fn new(key: u64) -> Composing {
        Composing {
            to: TextDocument::new(""),
            cc: TextDocument::new(""),
            bcc: TextDocument::new(""),
            subject: TextDocument::new(""),
            body: TextDocument::new(""),
            copies: false,
            files: Vec::new(),
            forwarded: Vec::new(),
            thread: None,
            in_reply_to: None,
            references: Vec::new(),
            draft: None,
            refusal: None,
            key,
        }
    }

    fn outgoing(&self, from: Address) -> google_mail::Result<Outgoing> {
        Ok(Outgoing {
            from,
            to: Address::typed_list(&self.to.text())?,
            cc: Address::typed_list(&self.cc.text())?,
            bcc: Address::typed_list(&self.bcc.text())?,
            subject: self.subject.text().trim().to_owned(),
            text: self.body.text(),
            in_reply_to: self.in_reply_to.clone(),
            references: self.references.clone(),
            enclosures: Vec::new(),
        })
    }
}

impl MailState {
    pub(crate) fn compose(&mut self) {
        self.drafts += 1;
        self.composing = Some(Composing::new(self.drafts));
        self.composed.changed();
    }

    pub(crate) fn answer(&mut self, message: &Message, answer: Answer) {
        let Some(session) = &self.session else {
            return;
        };
        let me = session.profile.email.to_lowercase();
        self.drafts += 1;
        let mut composing = Composing::new(self.drafts);
        let others = |addresses: &[Address]| {
            addresses
                .iter()
                .filter(|address| address.email.to_lowercase() != me)
                .map(Address::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        };
        let sender = match message.reply_to.is_empty() {
            true => std::slice::from_ref(&message.from),
            false => message.reply_to.as_slice(),
        };
        let when = format_timestamp(message.date_ms / MILLIS);
        let quoted = message.quotable();
        match answer {
            Answer::Reply | Answer::ReplyAll => {
                composing.to.set_text(match others(sender).is_empty() {
                    true => others(&message.to),
                    false => others(sender),
                });
                if answer == Answer::ReplyAll {
                    let copied = [others(&message.to), others(&message.cc)]
                        .into_iter()
                        .filter(|list| !list.is_empty())
                        .collect::<Vec<_>>()
                        .join(", ");
                    composing.copies = !copied.is_empty();
                    composing.cc.set_text(copied);
                }
                composing
                    .subject
                    .set_text(prefixed(REPLY, &message.subject));
                let mut body = format!(
                    "\n\n{}\n",
                    tr!(
                        "google.mail.compose.wrote",
                        time = when.as_str(),
                        sender = message.from.label()
                    )
                );
                for line in quoted.lines() {
                    body.push_str(match line.is_empty() {
                        true => ">",
                        false => "> ",
                    });
                    body.push_str(line);
                    body.push('\n');
                }
                composing.body.set_text(body);
                composing.thread = Some(message.thread_id.clone());
                composing.in_reply_to = Some(message.message_id.clone());
                composing.references = message.references.clone();
                composing.references.push(message.message_id.clone());
            }
            Answer::Forward => {
                composing
                    .subject
                    .set_text(prefixed(FORWARD, &message.subject));
                composing.body.set_text(format!(
                    "\n\n{}\n{}\n\n{quoted}\n",
                    tr!("google.mail.compose.forwarded"),
                    tr!(
                        "google.mail.compose.forwarded-head",
                        sender = message.from.to_string().as_str(),
                        time = when.as_str(),
                        subject = message.subject.as_str()
                    )
                ));
                composing.forwarded = message.files().cloned().collect();
            }
        }
        composing.body.select(0, 0);
        self.composing = Some(composing);
        self.composed.changed();
    }

    pub(crate) fn edit_draft(&mut self, message: &Message) {
        let Some(session) = &self.session else {
            return;
        };
        self.drafts += 1;
        let composing = Composing::new(self.drafts);
        let listed = |addresses: &[Address]| {
            addresses
                .iter()
                .map(Address::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        };
        composing.to.set_text(listed(&message.to));
        composing.cc.set_text(listed(&message.cc));
        composing.subject.set_text(&message.subject);
        composing.body.set_text(message.quotable());
        let key = composing.key;
        self.composing = Some(Composing {
            copies: !message.cc.is_empty(),
            thread: Some(message.thread_id.clone()),
            references: message.references.clone(),
            ..composing
        });
        self.composed.changed();
        let mail = session.mail.clone();
        let message_id = message.id.clone();
        spawn(
            async move { mail.draft_for(&message_id).await },
            move |mail: &mut MailState, draft: google_mail::Result<Option<google_mail::Draft>>| {
                match draft {
                    Ok(Some(draft)) => {
                        if let Some(open) = mail.composing.as_mut().filter(|open| open.key == key) {
                            open.draft = Some(draft.id);
                        }
                        skip_view();
                    }
                    Ok(None) => skip_view(),
                    Err(error) => mail.failed(&error),
                }
            },
        );
    }

    pub(crate) fn toggle_copies(&mut self) {
        if let Some(composing) = &mut self.composing {
            composing.copies = !composing.copies;
            self.composed.changed();
        }
    }

    pub(crate) fn attach_files(&mut self) {
        let parent = window_identifier();
        spawn_blocking(
            move || {
                Chooser::open(tr!("google.mail.compose.attach").to_string())
                    .parent(parent)
                    .multiple(true)
                    .run()
                    .map(|chosen| match chosen {
                        ChooserResponse::Files(paths) => MailState::picked(paths),
                        ChooserResponse::Cancelled => Vec::new(),
                    })
            },
            |mail: &mut MailState, chosen: rikka_dialog::Result<Vec<Picked>>| {
                let (Ok(picked), Some(composing)) = (chosen, &mut mail.composing) else {
                    return skip_view();
                };
                if picked.is_empty() {
                    return skip_view();
                }
                for file in picked {
                    if !composing.files.iter().any(|known| known.path == file.path) {
                        composing.files.push(file);
                    }
                }
                mail.composed.changed();
            },
        );
    }

    fn picked(paths: Vec<PathBuf>) -> Vec<Picked> {
        paths
            .into_iter()
            .filter_map(|path| match std::fs::metadata(&path) {
                Ok(metadata) => Some(Picked {
                    path,
                    size: metadata.len(),
                }),
                Err(error) => {
                    tracing::warn!("failed to read {}: {error}", path.display());
                    None
                }
            })
            .collect()
    }

    pub(crate) fn detach_file(&mut self, index: usize) {
        if let Some(composing) = &mut self.composing {
            let files = composing.files.len();
            match index.checked_sub(files) {
                Some(forwarded) if forwarded < composing.forwarded.len() => {
                    composing.forwarded.remove(forwarded);
                }
                None if index < files => {
                    composing.files.remove(index);
                }
                _ => {}
            }
            self.composed.changed();
        }
    }

    pub(crate) fn discard(&mut self) {
        let Some(composing) = self.composing.take() else {
            return;
        };
        self.composed.changed();
        if let (Some(session), Some(draft)) = (&self.session, composing.draft) {
            let mail = session.mail.clone();
            spawn(
                async move { mail.discard_draft(&draft).await },
                |mail: &mut MailState, discarded: google_mail::Result<()>| match discarded {
                    Ok(()) => mail.refresh(),
                    Err(error) => mail.failed(&error),
                },
            );
        }
    }

    pub(crate) fn send(&mut self) {
        self.deliver(Delivery::Send);
    }

    pub(crate) fn keep_draft(&mut self) {
        self.deliver(Delivery::Keep);
    }

    pub(crate) fn set_aside(&mut self) {
        let written = self.composing.as_ref().is_some_and(|composing| {
            [&composing.to, &composing.subject, &composing.body]
                .iter()
                .any(|document| !document.text().trim().is_empty())
        });
        match written {
            true => self.deliver(Delivery::SetAside),
            false => self.discard(),
        }
    }

    fn deliver(&mut self, delivery: Delivery) {
        let (Some(session), Some(composing)) = (&self.session, &mut self.composing) else {
            return;
        };
        if *self.sending {
            return;
        }
        let send = delivery == Delivery::Send;
        let from = Address::new(session.profile.name.clone(), session.profile.email.clone());
        let outgoing = match composing.outgoing(from) {
            Ok(outgoing) if send && !outgoing.has_recipient() => {
                composing.refusal = Some(tr!("google.mail.compose.no-recipient").to_string());
                return self.composed.changed();
            }
            Ok(outgoing) => outgoing,
            Err(error) => {
                composing.refusal = Some(view::failure(&error));
                return self.composed.changed();
            }
        };
        composing.refusal = None;
        self.composed.changed();
        self.sending.set(true);
        let mail = session.mail.clone();
        let files = composing.files.clone();
        let forwarded = composing.forwarded.clone();
        let thread = composing.thread.clone();
        let draft = composing.draft.clone();
        let key = composing.key;
        spawn(
            async move {
                let mut outgoing = outgoing;
                if !files.is_empty() {
                    let types = rikka_mime::Database::load();
                    for picked in files {
                        let content = std::fs::read(&picked.path)
                            .map_err(|error| format!("{}: {error}", picked.path.display()))?;
                        let mime = types.for_content(&content, &types.for_name(&picked.path));
                        outgoing.enclosures.push(Enclosure {
                            name: file_name(&picked.path),
                            mime: mime.as_str().to_owned(),
                            content,
                        });
                    }
                }
                for attachment in forwarded {
                    let content = mail
                        .attachment(&attachment.message_id, &attachment.attachment_id)
                        .await
                        .map_err(|error| view::failure(&error))?;
                    outgoing.enclosures.push(Enclosure {
                        name: attachment.name,
                        mime: attachment.mime,
                        content,
                    });
                }
                let failed = |error: google_mail::Error| view::failure(&error);
                match send {
                    true => {
                        mail.send(&outgoing, thread.as_deref())
                            .await
                            .map_err(failed)?;
                        if let Some(draft) = &draft {
                            mail.discard_draft(draft).await.map_err(failed)?;
                        }
                        Ok(None)
                    }
                    false => mail
                        .save_draft(&outgoing, thread.as_deref(), draft.as_deref())
                        .await
                        .map(|draft| Some(draft.id))
                        .map_err(failed),
                }
            },
            move |mail: &mut MailState, delivered: Result<Option<String>, String>| {
                mail.sending.set(false);
                match delivered {
                    Ok(None) => {
                        if mail.composing.as_ref().is_some_and(|open| open.key == key) {
                            mail.composing = None;
                            mail.composed.changed();
                        }
                        mail.notify(tr!("google.mail.compose.sent").to_string(), false);
                        mail.refresh();
                    }
                    Ok(Some(draft)) => {
                        let open = mail.composing.as_ref().is_some_and(|open| open.key == key);
                        if open && delivery == Delivery::SetAside {
                            mail.composing = None;
                            mail.composed.changed();
                        } else if let Some(open) = mail.composing.as_mut().filter(|_| open) {
                            open.draft = Some(draft);
                        }
                        mail.notify(tr!("google.mail.compose.saved").to_string(), false);
                        mail.refresh();
                    }
                    Err(reason) => {
                        if let Some(open) = mail.composing.as_mut().filter(|open| open.key == key) {
                            open.refusal = Some(reason);
                            mail.composed.changed();
                        }
                    }
                }
            },
        );
    }
}

pub(crate) fn sheet(composing: &Composing, sending: bool) -> Vec<Element> {
    let body = column! {
        recipients(tr!("google.mail.compose.to"), &composing.to, !composing.copies, true);
        if composing.copies {
            recipients(tr!("google.mail.compose.cc"), &composing.cc, false, false);
            recipients(tr!("google.mail.compose.bcc"), &composing.bcc, false, false);
        }
        text_field(&composing.subject)
            .placeholder(tr!("google.mail.compose.subject"))
            .fill_width();
        text_editor(&composing.body)
            .fits_text(BODY)
            .autofocus(composing.in_reply_to.is_some())
            .fill_width();
        enclosures(composing);
        if let Some(refusal) = &composing.refusal {
            text(refusal.clone()).caption().danger();
        }
    }
    .gap(Space::SNUG)
    .fill_width();
    dialog(
        match composing.in_reply_to {
            Some(_) => tr!("google.mail.compose.reply"),
            None => tr!("google.mail.compose.new"),
        },
        body,
    )
    .aside(
        button(icon(MaterialSymbol::ATTACH_FILE).extent(Length::px(18.0)))
            .ghost()
            .icon_only()
            .on_press(MailState::attach_files)
            .tooltip(tr!("google.mail.compose.attach")),
    )
    .cancel(tr!("google.mail.compose.discard"), MailState::discard)
    .action(
        button(text(tr!("google.mail.compose.keep")))
            .ghost()
            .enabled(!sending)
            .on_press(MailState::keep_draft),
    )
    .action(
        button(text(tr!("google.mail.compose.send")))
            .primary()
            .loading(sending)
            .enabled(!sending)
            .on_press(MailState::send),
    )
    .on_chord(Key::Enter, Modifiers::CONTROL, MailState::send)
    .on_dismiss(MailState::set_aside)
    .width(WIDTH)
    .keyed((KEY, composing.key))
    .centered_on(STAGE)
    .with_scrim()
}

fn recipients(label: Label, document: &TextDocument, expands: bool, focused: bool) -> impl View {
    row! {
        text(label).caption().muted().width(44.0);
        text_field(document).autofocus(focused).grow(1.0);
        if expands {
            button(text(tr!("google.mail.compose.copies")).caption())
                .ghost()
                .small()
                .on_press(MailState::toggle_copies);
        }
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}

fn enclosures(composing: &Composing) -> Option<Element> {
    let named = composing
        .files
        .iter()
        .map(|picked| (file_name(&picked.path), picked.size))
        .chain(
            composing
                .forwarded
                .iter()
                .map(|attachment| (attachment.name.clone(), attachment.size)),
        );
    let chips: Vec<Element> = named
        .enumerate()
        .map(|(index, (name, size))| {
            container(
                row! {
                    icon(MaterialSymbol::ATTACH_FILE).extent(Length::px(14.0));
                    text(name).caption().no_wrap();
                    text(format_bytes(size)).caption().muted().no_wrap();
                    button(icon(MaterialSymbol::CLOSE).extent(Length::px(12.0)))
                        .ghost()
                        .small()
                        .icon_only()
                        .on_press(move |mail: &mut MailState| mail.detach_file(index));
                }
                .gap(Space::TIGHT)
                .align_items(Align::Center),
            )
            .padding(Inset::xy(Space::SNUG, Space::HAIR))
            .radius(Corner::Item)
            .background(palette().surface_variant)
            .any()
        })
        .collect();
    (!chips.is_empty()).then(|| {
        row_of(chips)
            .gap(Space::TIGHT)
            .wrap_items()
            .fill_width()
            .any()
    })
}

pub(crate) fn is_draft(message: &Message) -> bool {
    message.has(DRAFT)
}

fn prefixed(prefix: &str, subject: &str) -> String {
    match subject
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
    {
        true => subject.to_owned(),
        false => format!("{prefix}{subject}"),
    }
}

fn file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}
