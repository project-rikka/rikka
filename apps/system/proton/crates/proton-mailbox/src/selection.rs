use rikka_proton_mail::{ConversationId, LabelId, MailClient, MessageId};

use crate::error::Result;
use crate::row::RowId;

#[derive(Debug, Default)]
pub(crate) struct Selection {
    messages: Vec<MessageId>,
    conversations: Vec<ConversationId>,
}

impl Selection {
    pub(crate) fn from_ids(ids: &[RowId]) -> Selection {
        ids.iter().fold(Selection::default(), |mut selection, id| {
            match id {
                RowId::Message(id) => selection.messages.push(id.clone()),
                RowId::Conversation(id) => selection.conversations.push(id.clone()),
            }
            selection
        })
    }

    pub(crate) async fn mark(
        &self,
        client: &MailClient,
        read: bool,
        folder: &LabelId,
    ) -> Result<()> {
        if !self.messages.is_empty() {
            let messages = client.message_batch(&self.messages);
            match read {
                true => messages.mark_read().await?,
                false => messages.mark_unread().await?,
            };
        }
        if !self.conversations.is_empty() {
            let conversations = client.conversation_batch(&self.conversations);
            match read {
                true => conversations.mark_read().await?,
                false => conversations.in_label(folder).mark_unread().await?,
            };
        }
        Ok(())
    }

    pub(crate) async fn label(&self, client: &MailClient, label: &LabelId, on: bool) -> Result<()> {
        if !self.messages.is_empty() {
            let messages = client.message_batch(&self.messages);
            match on {
                true => messages.label(label).await?,
                false => messages.unlabel(label).await?,
            };
        }
        if !self.conversations.is_empty() {
            let conversations = client.conversation_batch(&self.conversations);
            match on {
                true => conversations.label(label).await?,
                false => conversations.unlabel(label).await?,
            };
        }
        Ok(())
    }

    pub(crate) async fn delete(&self, client: &MailClient, folder: &LabelId) -> Result<()> {
        if !self.messages.is_empty() {
            client.message_batch(&self.messages).delete().await?;
        }
        if !self.conversations.is_empty() {
            client
                .conversation_batch(&self.conversations)
                .in_label(folder)
                .delete()
                .await?;
        }
        Ok(())
    }
}
