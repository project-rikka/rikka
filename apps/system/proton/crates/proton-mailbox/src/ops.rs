use futures_util::future::{join, join_all, try_join};
use rikka_proton_api::offload;
use rikka_proton_mail::{
    Conversation, LabelId, LabelQuery, LabelType, MailAddress, MailClient, MessageId,
    MessageMetadata, MimeType, Query, QueryKind, Sort, SystemLabel,
};

use crate::ask::Ask;
use crate::draft::{Draft, addresses};
use crate::error::{Error, Result};
use crate::folder::{Folder, arranged};
use crate::opened::{MessageAttachment, OpenedMessage, OpenedThread};
use crate::row::{Row, RowId};
use crate::selection::Selection;

const THREAD: usize = 25;

#[derive(Clone, Debug, Default)]
pub struct Listing {
    pub rows: Vec<Row>,
    pub more: bool,
}

pub async fn folders(client: &MailClient, grouped: bool) -> Result<Vec<Folder>> {
    let labels = folder_labels(client).list();
    let counts = async {
        match grouped {
            true => client.conversation_counts().await,
            false => client.message_counts().await,
        }
    };
    let (labels, counts) = try_join(labels, counts).await?;

    Ok(arranged(&labels, &counts))
}

#[must_use]
pub fn cached_folders(client: &MailClient, grouped: bool) -> Option<Vec<Folder>> {
    let labels = folder_labels(client).cached()?;
    let counts = match grouped {
        true => client.cached_conversation_counts(),
        false => client.cached_message_counts(),
    };

    Some(arranged(&labels, &counts.unwrap_or_default()))
}

pub async fn page(client: &MailClient, ask: &Ask) -> Result<Listing> {
    Ok(match ask.threaded() {
        true => threads(&asked(client.conversations(), ask).list().await?, ask),
        false => letters(&asked(client.messages(), ask).list().await?, ask),
    })
}

#[must_use]
pub fn cached_page(client: &MailClient, ask: &Ask) -> Option<Listing> {
    match ask.threaded() {
        true => Some(threads(&asked(client.conversations(), ask).cached()?, ask)),
        false => Some(letters(&asked(client.messages(), ask).cached()?, ask)),
    }
}

fn folder_labels(client: &MailClient) -> LabelQuery {
    client.labels().kinds([LabelType::Folder, LabelType::Label])
}

fn asked<K: QueryKind>(query: Query<K>, ask: &Ask) -> Query<K> {
    let mut query = query
        .label(&ask.folder)
        .sort(Sort::Time)
        .descending(true)
        .page(ask.page)
        .page_size(ask.size);
    if let Some(keyword) = &ask.keyword {
        query = query.keyword(keyword).auto_wildcard(true);
    }
    if ask.unread {
        query = query.unread(true);
    }
    query
}

fn threads(page: &[Conversation], ask: &Ask) -> Listing {
    Listing {
        more: reached(page.len(), ask.size),
        rows: page
            .iter()
            .map(|conversation| Row::of_conversation(conversation, &ask.folder, ask.recipients()))
            .collect(),
    }
}

fn letters(page: &[MessageMetadata], ask: &Ask) -> Listing {
    Listing {
        more: reached(page.len(), ask.size),
        rows: page
            .iter()
            .map(|message| Row::of_message(message, ask.recipients()))
            .collect(),
    }
}

pub async fn read(client: &MailClient, row: &Row) -> Result<OpenedThread> {
    match &row.id {
        RowId::Message(message) => {
            let opened = one(client, message, row.senders.first().map(String::as_str)).await?;
            Ok(OpenedThread {
                id: row.id.clone(),
                subject: opened.subject.clone(),
                messages: vec![opened],
                thread: 1,
            })
        }
        RowId::Conversation(conversation) => {
            let warming = join_all(row.senders.iter().map(|sender| client.sender_keys(sender)));
            let (thread, _) = join(client.conversation(conversation), warming).await;
            let (conversation, mut messages) = thread?.into_parts();
            messages.sort_by_key(|message| (message.time(), message.order()));
            let latest = &messages[messages.len().saturating_sub(THREAD)..];

            let results = join_all(
                latest
                    .iter()
                    .map(|message| one(client, message.id(), Some(message.sender().address()))),
            )
            .await;
            let mut opened = Vec::new();
            let mut failure = None;
            for result in results {
                match result {
                    Ok(message) => opened.push(message),
                    Err(error) => failure = Some(error),
                }
            }

            match (opened.is_empty(), failure) {
                (true, Some(error)) => Err(error),
                _ => Ok(OpenedThread {
                    id: row.id.clone(),
                    subject: conversation.subject().to_owned(),
                    messages: opened,
                    thread: conversation.messages(),
                }),
            }
        }
    }
}

pub async fn attachment(client: &MailClient, file: &MessageAttachment) -> Result<Vec<u8>> {
    let opened = client.attachment(&file.source, &[]).await?;

    Ok(opened.into_bytes())
}

pub async fn mark_read(
    client: &MailClient,
    ids: &[RowId],
    read: bool,
    folder: &LabelId,
) -> Result<()> {
    Selection::from_ids(ids).mark(client, read, folder).await
}

pub async fn set_starred(client: &MailClient, ids: &[RowId], starred: bool) -> Result<()> {
    set_label(client, ids, &SystemLabel::Starred.id(), starred).await
}

pub async fn set_label(
    client: &MailClient,
    ids: &[RowId],
    label: &LabelId,
    on: bool,
) -> Result<()> {
    Selection::from_ids(ids).label(client, label, on).await
}

pub async fn move_rows(
    client: &MailClient,
    ids: &[RowId],
    from: &LabelId,
    to: &LabelId,
) -> Result<()> {
    set_label(client, ids, to, true).await?;
    if from != to && from.is_system() {
        set_label(client, ids, from, false).await?;
    }
    Ok(())
}

pub async fn trash(client: &MailClient, ids: &[RowId], from: &LabelId) -> Result<()> {
    move_rows(client, ids, from, &SystemLabel::Trash.id()).await
}
pub async fn delete(client: &MailClient, ids: &[RowId], folder: &LabelId) -> Result<()> {
    Selection::from_ids(ids).delete(client, folder).await
}

pub async fn save_draft(client: &MailClient, draft: &Draft) -> Result<MessageId> {
    let from = draft.from.clone().ok_or(Error::NoSender)?;

    let mut builder = client
        .draft()
        .sender(&from)
        .subject(&draft.subject)
        .body(&draft.body)
        .mime_type(MimeType::PlainText);
    for email in addresses(&draft.to) {
        builder = builder.to(MailAddress::new(email));
    }
    for email in addresses(&draft.cc) {
        builder = builder.cc(MailAddress::new(email));
    }
    for email in addresses(&draft.bcc) {
        builder = builder.bcc(MailAddress::new(email));
    }
    if let Some(answering) = &draft.answering
        && draft.id.is_none()
    {
        builder = builder.in_reply_to(&answering.message, answering.action);
    }

    let saved = match &draft.id {
        Some(id) => builder.update(id).await?,
        None => builder.create().await?,
    };
    let id = saved.id().clone();

    for file in &draft.files {
        client
            .upload(&id, &file.name, file.bytes.clone())
            .sender(&from)
            .mime_type(file.media_type.clone())
            .upload()
            .await?;
    }

    Ok(id)
}

pub async fn send(client: &MailClient, draft: &Draft) -> Result<MessageId> {
    let from = draft.from.clone().ok_or(Error::NoSender)?;
    let recipients = draft.recipients();
    if recipients.is_empty() {
        return Err(Error::NoRecipient);
    }

    let id = save_draft(client, draft).await?;
    let message = client.message(&id).await?;

    let mut send = client
        .send(&id)
        .sender(&from)
        .body(&draft.body)
        .mime_type(MimeType::PlainText);
    for email in recipients {
        send = send.to(email);
    }
    for attachment in message.attachments() {
        send = send.attachment_key(attachment.id(), client.attachment_session_key(attachment)?);
    }

    send.send().await?;
    Ok(id)
}

pub async fn discard_draft(client: &MailClient, draft: &MessageId) -> Result<()> {
    client
        .message_batch(std::slice::from_ref(draft))
        .delete()
        .await
        .map(drop)
        .map_err(Error::from)
}

async fn one(client: &MailClient, id: &MessageId, sender: Option<&str>) -> Result<OpenedMessage> {
    let opened = client.open_message(id, sender).await?;

    Ok(offload(move || {
        let shown = OpenedMessage::from_message(opened.message(), opened.decrypted());
        shown.words.warm_markdown_cache();
        shown
    })
    .await)
}

fn reached(count: usize, size: u32) -> bool {
    u32::try_from(count).unwrap_or(u32::MAX) >= size
}
