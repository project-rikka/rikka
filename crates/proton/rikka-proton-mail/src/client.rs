use std::sync::Arc;

use futures_util::future::join;
use rikka_proton_account::{Account, AccountKeys, Address, AddressId};
use rikka_proton_api::{Api, offload};
use rikka_proton_cache::Cache;
use rikka_proton_contacts::ContactsClient;
use rikka_proton_pgp::{Pgp, PrivateKey, PublicKey, SessionKey};
use serde::Deserialize;

use crate::attachment::Attachment;
use crate::batch::Batch;
use crate::cached;
use crate::conversation::{Conversation, ConversationThread};
use crate::count::LabelCount;
use crate::decrypted::{DecryptedAttachment, DecryptedMessage};
use crate::direct::DirectSend;
use crate::draft::DraftBuilder;
use crate::error::{Error, Result};
use crate::event::Event;
use crate::identifier::{AttachmentId, ConversationId, LabelId, MessageId};
use crate::import::Import;
use crate::label::Label;
use crate::label_draft::LabelDraft;
use crate::label_query::LabelQuery;
use crate::message::{Message, MessageMetadata};
use crate::message_ids::MessageIds;
use crate::mime::{MimeDecryptor, MimeMessage, Secret};
use crate::opened_message::OpenedMessage;
use crate::query::{ConversationQuery, MessageQuery};
use crate::search::Indexer;
use crate::send::SendBuilder;
use crate::settings::SettingsCell;
use crate::upload::AttachmentUpload;

pub(crate) const MESSAGES: &str = "mail/v4/messages";
pub(crate) const CONVERSATIONS: &str = "mail/v4/conversations";
pub(crate) const ATTACHMENTS: &str = "mail/v4/attachments";
pub(crate) const LABELS: &str = "core/v4/labels";
pub(crate) const PAGE: u32 = 150;

#[derive(Debug, Clone)]
pub struct MailClient {
    account: Account,
    keys: Arc<AccountKeys>,
    contacts: ContactsClient,
    settings: Arc<SettingsCell>,
}

impl MailClient {
    #[must_use]
    pub fn new(api: Api, keys: Arc<AccountKeys>) -> MailClient {
        MailClient::with_account(Account::new(api), keys)
    }

    #[must_use]
    pub fn with_pgp(api: Api, pgp: Pgp, keys: Arc<AccountKeys>) -> MailClient {
        MailClient::with_account(Account::with_pgp(api, pgp), keys)
    }

    #[must_use]
    pub fn for_account(account: &Account, keys: &Arc<AccountKeys>) -> MailClient {
        MailClient::with_account(account.clone(), Arc::clone(keys))
    }

    #[must_use]
    fn with_account(account: Account, keys: Arc<AccountKeys>) -> MailClient {
        MailClient {
            contacts: ContactsClient::for_account(&account, &keys),
            account,
            keys,
            settings: Arc::default(),
        }
    }

    #[must_use]
    pub fn with_keys(mut self, keys: &Arc<AccountKeys>) -> MailClient {
        self.keys = Arc::clone(keys);
        self.contacts = self.contacts.with_keys(keys);
        self
    }

    #[must_use]
    pub fn with_cache(mut self, cache: Cache) -> MailClient {
        self.account = self.account.with_cache(cache);
        let rebuilt = ContactsClient::for_account(&self.account, &self.keys);
        self.contacts = match self.contacts.key_transparency() {
            Some((transparency, activation)) => {
                rebuilt.with_key_transparency(transparency.clone(), activation)
            }
            None => rebuilt,
        };
        self
    }

    #[must_use]
    pub fn with_contacts(mut self, contacts: ContactsClient) -> MailClient {
        self.contacts = contacts;
        self
    }

    #[must_use]
    pub fn contacts(&self) -> &ContactsClient {
        &self.contacts
    }

    pub(crate) fn settings_cell(&self) -> &SettingsCell {
        &self.settings
    }

    #[must_use]
    pub fn cache(&self) -> Option<&Cache> {
        self.account.cache()
    }

    #[must_use]
    pub fn account(&self) -> &Account {
        &self.account
    }

    #[must_use]
    pub fn api(&self) -> &Api {
        self.account.api()
    }

    #[must_use]
    pub fn pgp(&self) -> &Pgp {
        self.account.pgp()
    }

    #[must_use]
    pub fn keys(&self) -> &AccountKeys {
        &self.keys
    }

    pub fn labels(&self) -> LabelQuery {
        LabelQuery::new(self.clone())
    }

    pub fn new_label(&self, name: impl Into<String>) -> LabelDraft {
        LabelDraft::new(self.clone(), name)
    }

    pub async fn label(&self, id: &LabelId) -> Result<Label> {
        self.labels()
            .list()
            .await?
            .into_iter()
            .find(|label| label.id() == id)
            .ok_or_else(|| Error::NoSuchLabel(id.clone()))
    }

    pub async fn delete_label(&self, id: &LabelId) -> Result<()> {
        if id.is_system() {
            return Err(Error::SystemLabel(id.clone()));
        }

        self.api()
            .delete(format!("{LABELS}/{id}"))
            .send_unit()
            .await?;

        Ok(())
    }

    pub fn messages(&self) -> MessageQuery {
        MessageQuery::new(self.clone())
    }

    pub async fn message(&self, id: &MessageId) -> Result<Message> {
        let response: Message = self
            .api()
            .get(format!("{MESSAGES}/{id}"))
            .send_field("Message")
            .await?;

        if let Some(cache) = self.cache() {
            cache.put(
                &cached::MESSAGES,
                response.id().as_str(),
                response.metadata().clone(),
            );
        }
        Ok(response)
    }

    pub async fn message_counts(&self) -> Result<Vec<LabelCount>> {
        self.counts(MESSAGES).await
    }

    pub fn message_batch<'a>(&'a self, ids: &'a [MessageId]) -> Batch<'a, MessageId> {
        Batch::new(self, ids)
    }

    pub fn conversations(&self) -> ConversationQuery {
        ConversationQuery::new(self.clone())
    }

    pub async fn conversation(&self, id: &ConversationId) -> Result<ConversationThread> {
        let response: ConversationResponse = self
            .api()
            .get(format!("{CONVERSATIONS}/{id}"))
            .send()
            .await?;

        if let Some(cache) = self.cache() {
            let mut batch = cache.batch();
            batch.put(
                &cached::CONVERSATIONS,
                response.conversation.id().as_str(),
                response.conversation.clone(),
            );
            for message in &response.messages {
                batch.put(&cached::MESSAGES, message.id().as_str(), message.clone());
            }
            batch.commit();
        }

        Ok(ConversationThread::new(
            response.conversation,
            response.messages,
        ))
    }

    pub async fn conversation_counts(&self) -> Result<Vec<LabelCount>> {
        self.counts(CONVERSATIONS).await
    }

    #[must_use]
    pub fn cached_message_counts(&self) -> Option<Vec<LabelCount>> {
        self.cache()?.get(&cached::COUNTS, MESSAGES)
    }

    #[must_use]
    pub fn cached_conversation_counts(&self) -> Option<Vec<LabelCount>> {
        self.cache()?.get(&cached::COUNTS, CONVERSATIONS)
    }

    async fn counts(&self, collection: &'static str) -> Result<Vec<LabelCount>> {
        let response: Vec<LabelCount> = self
            .api()
            .get(format!("{collection}/count"))
            .send_field_or_default("Counts")
            .await?;
        if let Some(cache) = self.cache() {
            cache.put(&cached::COUNTS, collection, response.clone());
        }

        Ok(response)
    }

    pub fn conversation_batch<'a>(
        &'a self,
        ids: &'a [ConversationId],
    ) -> Batch<'a, ConversationId> {
        Batch::new(self, ids)
    }

    pub fn draft(&self) -> DraftBuilder {
        DraftBuilder::new(self.clone())
    }

    pub fn send(&self, message: &MessageId) -> SendBuilder {
        SendBuilder::new(self.clone(), message.clone())
    }

    pub fn direct(&self) -> DirectSend {
        DirectSend::new(self.clone())
    }

    pub fn message_ids(&self) -> MessageIds {
        MessageIds::new(self.clone())
    }

    pub fn import(&self) -> Import {
        Import::new(self.clone())
    }

    pub fn indexer(&self) -> Indexer {
        Indexer::new(self.clone())
    }

    pub fn upload(
        &self,
        message: &MessageId,
        name: impl Into<String>,
        data: impl Into<Vec<u8>>,
    ) -> AttachmentUpload {
        AttachmentUpload::new(self.clone(), message.clone(), name, data)
    }

    pub async fn attachment_bytes(&self, id: &AttachmentId) -> Result<Vec<u8>> {
        let bytes = self
            .api()
            .get(format!("{ATTACHMENTS}/{id}"))
            .send_bytes()
            .await?;

        Ok(bytes.to_vec())
    }

    pub async fn attachment(
        &self,
        attachment: &Attachment,
        verifiers: &[PublicKey],
    ) -> Result<DecryptedAttachment> {
        let data = self.attachment_bytes(attachment.id()).await?;
        let client = self.clone();
        let attachment = attachment.clone();
        let verifiers = verifiers.to_vec();

        offload(move || client.decrypt_attachment(&attachment, &data, &verifiers)).await
    }

    pub fn decrypt_attachment(
        &self,
        attachment: &Attachment,
        data: &[u8],
        verifiers: &[PublicKey],
    ) -> Result<DecryptedAttachment> {
        let session_key = self.attachment_session_key(attachment)?;
        let decrypted_message = self
            .pgp()
            .decrypt()
            .with_session_key(session_key)
            .verified_by_keys(verifiers.to_vec())
            .message(data)?;

        let signature = match attachment.signature_bytes()? {
            Some(detached) => self
                .pgp()
                .verify()
                .with_keys(verifiers.to_vec())
                .detached(decrypted_message.data(), detached),
            None => decrypted_message.signature().clone(),
        };

        Ok(DecryptedAttachment::new(
            attachment,
            decrypted_message,
            signature,
        ))
    }

    pub fn attachment_session_key(&self, attachment: &Attachment) -> Result<SessionKey> {
        Ok(self
            .pgp()
            .decrypt()
            .with_keys(self.all_address_keys()?)
            .session_key(attachment.key_packet_bytes()?)?)
    }

    pub async fn delete_attachment(&self, id: &AttachmentId) -> Result<()> {
        self.api()
            .delete(format!("{ATTACHMENTS}/{id}"))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn open_message(
        &self,
        id: &MessageId,
        sender: Option<&str>,
    ) -> Result<OpenedMessage> {
        let prefetch_sender_keys = async {
            if let Some(email) = sender {
                drop(self.sender_keys(email).await);
            }
        };
        let (message, ()) = join(self.message(id), prefetch_sender_keys).await;

        self.open_fetched(message?).await
    }

    pub async fn decrypt_message(&self, message: &Message) -> Result<DecryptedMessage> {
        let (_, decrypted) = self.open_fetched(message.clone()).await?.into_parts();

        Ok(decrypted)
    }

    async fn open_fetched(&self, message: Message) -> Result<OpenedMessage> {
        let verifiers = self.sender_verifiers(&message).await?;
        let client = self.clone();

        offload(move || {
            let decrypted = client.decrypt_message_with(&message, &verifiers)?;
            Ok(OpenedMessage::new(message, decrypted))
        })
        .await
    }

    async fn sender_verifiers(&self, message: &Message) -> Result<Arc<[PublicKey]>> {
        match message.sender().address() {
            "" => Ok(Arc::from([])),
            sender => self.sender_keys(sender).await,
        }
    }

    pub fn decrypt_message_with(
        &self,
        message: &Message,
        verifiers: &[PublicKey],
    ) -> Result<DecryptedMessage> {
        let keys = self.reading_keys(message.address_id())?;
        let decrypted_message = self
            .pgp()
            .decrypt()
            .with_keys(keys.clone())
            .verified_by_keys(verifiers.to_vec())
            .message(message.body())?;

        let mime = message.mime_type().is_mime().then(|| {
            MimeDecryptor::new(self.pgp(), Secret::Keys(&keys), verifiers)
                .parse(decrypted_message.data())
        });

        Ok(DecryptedMessage::new(message, decrypted_message, mime))
    }

    pub fn read_mime(
        &self,
        address: &AddressId,
        raw: impl AsRef<[u8]>,
        verifiers: &[PublicKey],
    ) -> Result<MimeMessage> {
        let keys = self.reading_keys(address)?;

        Ok(MimeDecryptor::new(self.pgp(), Secret::Keys(&keys), verifiers).parse(raw.as_ref()))
    }

    pub async fn sender_keys(&self, email: &str) -> Result<Arc<[PublicKey]>> {
        Ok(self.contacts.verifying_keys(email).await?)
    }

    pub fn apply_event(&self, event: &Event) {
        if let Some(cache) = self.cache() {
            cached::apply(cache, event);
        }
    }

    pub fn forget_cached(&self) {
        if let Some(cache) = self.cache() {
            cached::clear(cache);
        }
    }

    #[must_use]
    pub fn cached_message(&self, id: &MessageId) -> Option<MessageMetadata> {
        self.cache()?.get(&cached::MESSAGES, id.as_str())
    }

    #[must_use]
    pub fn cached_conversation(&self, id: &ConversationId) -> Option<Conversation> {
        self.cache()?.get(&cached::CONVERSATIONS, id.as_str())
    }

    pub(crate) fn sending_key(&self, address: &AddressId) -> Result<(Address, PrivateKey)> {
        Ok((
            self.keys.known_address(address)?.clone(),
            self.keys.signer(address)?.clone(),
        ))
    }

    pub(crate) fn reading_keys(&self, address: &AddressId) -> Result<Vec<PrivateKey>> {
        match self.keys.address_keys(address) {
            Some(ring) => Ok(ring.private_keys().to_vec()),
            None => self.all_address_keys(),
        }
    }

    pub(crate) fn all_address_keys(&self) -> Result<Vec<PrivateKey>> {
        let keys: Vec<PrivateKey> = self
            .keys
            .addresses()
            .iter()
            .filter_map(|address| self.keys.address_keys(address.id()))
            .flat_map(|ring| ring.private_keys().iter().cloned())
            .collect();

        match keys.is_empty() {
            true => Err(Error::NoUnlockedKey),
            false => Ok(keys),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ConversationResponse {
    #[serde(rename = "Conversation")]
    conversation: Conversation,
    #[serde(rename = "Messages", default)]
    messages: Vec<MessageMetadata>,
}
