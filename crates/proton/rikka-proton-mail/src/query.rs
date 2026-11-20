use jiff::Timestamp;
use std::fmt;
use std::marker::PhantomData;

use rikka_proton_account::AddressId;
use rikka_proton_api::flag;
use rikka_proton_cache::{Cache, Record, Table};

use crate::cached;
use crate::client::{CONVERSATIONS, MESSAGES, MailClient};
use crate::conversation::Conversation;
use crate::error::Result;
use crate::identifier::{ConversationId, LabelId, MessageId};
use crate::listing::Listing;
use crate::message::MessageMetadata;
use crate::sort::Sort;

mod kind {
    use super::{LabelId, Record, Table, fmt};

    pub trait QueryKind: Copy + fmt::Debug + Send + Sync + 'static {
        type Item: Record;
        type Id: Record + fmt::Display;

        const PATH: &'static str;
        const FIELD: &'static str;
        const ITEMS: Table<Self::Item>;
        const PAGES: Table<Vec<Self::Id>>;

        fn id(item: &Self::Item) -> &Self::Id;

        fn key(id: &Self::Id) -> &str;

        fn labelled(item: &Self::Item, label: &LabelId) -> bool;
    }
}

pub use kind::QueryKind;

#[derive(Debug, Clone, Copy)]
pub struct Messages;

#[derive(Debug, Clone, Copy)]
pub struct Conversations;

pub type MessageQuery = Query<Messages>;
pub type ConversationQuery = Query<Conversations>;

#[must_use]
pub struct Query<K> {
    listing: Listing,
    kind: PhantomData<K>,
}

impl<K> Clone for Query<K> {
    fn clone(&self) -> Query<K> {
        Query {
            listing: self.listing.clone(),
            kind: PhantomData,
        }
    }
}

impl<K: QueryKind> fmt::Debug for Query<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Query")
            .field("listing", &self.listing)
            .finish()
    }
}

impl<K: QueryKind> Query<K> {
    pub(crate) fn new(client: MailClient) -> Query<K> {
        Query {
            listing: Listing::new(client, K::PATH),
            kind: PhantomData,
        }
    }

    pub fn page(self, page: u32) -> Self {
        self.with(|listing| listing.page(page))
    }

    pub fn page_size(self, size: u32) -> Self {
        self.with(|listing| listing.page_size(size))
    }

    pub fn limit(self, limit: u32) -> Self {
        self.set("Limit", limit.to_string())
    }

    pub fn label(self, label: &LabelId) -> Self {
        self.set("LabelID", label.to_string())
    }

    pub fn address(self, address: &AddressId) -> Self {
        self.set("AddressID", address.to_string())
    }

    pub fn ids(self, ids: impl IntoIterator<Item = K::Id>) -> Self {
        self.with(|listing| {
            ids.into_iter().fold(listing, |listing, id| {
                listing.repeat("ID[]", id.to_string())
            })
        })
    }

    pub fn sort(self, sort: Sort) -> Self {
        self.set("Sort", sort.as_str().to_owned())
    }

    pub fn descending(self, descending: bool) -> Self {
        self.set("Desc", flag::to_query_value(descending))
    }

    pub fn begin(self, at: Timestamp) -> Self {
        self.set("Begin", at.as_second().to_string())
    }

    pub fn end(self, at: Timestamp) -> Self {
        self.set("End", at.as_second().to_string())
    }

    pub fn begin_id(self, id: &K::Id) -> Self {
        self.set("BeginID", id.to_string())
    }

    pub fn end_id(self, id: &K::Id) -> Self {
        self.set("EndID", id.to_string())
    }

    pub fn anchor(self, at: Timestamp, id: &K::Id) -> Self {
        self.set("Anchor", at.as_second().to_string())
            .set("AnchorID", id.to_string())
    }

    pub fn keyword(self, keyword: impl Into<String>) -> Self {
        self.set("Keyword", keyword.into())
    }

    pub fn subject(self, subject: impl Into<String>) -> Self {
        self.set("Subject", subject.into())
    }

    pub fn from(self, from: impl Into<String>) -> Self {
        self.set("From", from.into())
    }

    pub fn to(self, to: impl Into<String>) -> Self {
        self.set("To", to.into())
    }

    pub fn unread(self, unread: bool) -> Self {
        self.set("Unread", flag::to_query_value(unread))
    }

    pub fn with_attachments(self, attachments: bool) -> Self {
        self.set("Attachments", flag::to_query_value(attachments))
    }

    pub fn auto_wildcard(self, wildcard: bool) -> Self {
        self.set("AutoWildcard", flag::to_query_value(wildcard))
    }

    pub async fn list(self) -> Result<Vec<K::Item>> {
        let listed = self.listing.list(K::FIELD).await?;
        self.store_in_cache(false, &listed);
        Ok(listed)
    }

    pub async fn all(self) -> Result<Vec<K::Item>> {
        let listed = self.listing.all(K::FIELD).await?;
        self.store_in_cache(true, &listed);
        Ok(listed)
    }

    #[must_use]
    pub fn cached(&self) -> Option<Vec<K::Item>> {
        self.load_from_cache(false)
    }

    #[must_use]
    pub fn cached_all(&self) -> Option<Vec<K::Item>> {
        self.load_from_cache(true)
    }

    fn store_in_cache(&self, all_pages: bool, listed: &[K::Item]) {
        let Some(cache) = self.listing.client().cache() else {
            return;
        };
        let mut batch = cache.batch();
        for item in listed {
            batch.put(&K::ITEMS, K::key(K::id(item)), item.clone());
        }
        if !self.listing.is_cacheable() {
            batch.commit();
            return;
        }
        batch.put(
            &K::PAGES,
            &self.listing.cache_key(all_pages),
            listed.iter().map(|item| K::id(item).clone()).collect(),
        );
        batch.commit();
    }

    fn load_from_cache(&self, all_pages: bool) -> Option<Vec<K::Item>> {
        let cache: &Cache = self.listing.client().cache()?;
        let ids = cache.get(&K::PAGES, &self.listing.cache_key(all_pages))?;
        let label = self.listing.label();
        Some(
            ids.iter()
                .filter_map(|id| cache.get(&K::ITEMS, K::key(id)))
                .filter(|item| label.as_ref().is_none_or(|label| K::labelled(item, label)))
                .collect(),
        )
    }

    fn set(self, name: &'static str, value: String) -> Self {
        self.with(|listing| listing.set(name, value))
    }

    fn with(self, change: impl FnOnce(Listing) -> Listing) -> Self {
        Query {
            listing: change(self.listing),
            kind: PhantomData,
        }
    }
}

impl Query<Messages> {
    pub fn conversation(self, conversation: &ConversationId) -> Self {
        self.set("ConversationID", conversation.to_string())
    }

    pub fn cc(self, cc: impl Into<String>) -> Self {
        self.set("CC", cc.into())
    }

    pub fn bcc(self, bcc: impl Into<String>) -> Self {
        self.set("BCC", bcc.into())
    }

    pub fn external_id(self, id: impl Into<String>) -> Self {
        self.set("ExternalID", id.into())
    }
}

impl QueryKind for Messages {
    type Item = MessageMetadata;
    type Id = MessageId;

    const PATH: &'static str = MESSAGES;
    const FIELD: &'static str = "Messages";
    const ITEMS: Table<MessageMetadata> = cached::MESSAGES;
    const PAGES: Table<Vec<MessageId>> = cached::MESSAGE_PAGES;

    fn id(item: &MessageMetadata) -> &MessageId {
        item.id()
    }

    fn key(id: &MessageId) -> &str {
        id.as_str()
    }

    fn labelled(item: &MessageMetadata, label: &LabelId) -> bool {
        item.has_label(label)
    }
}

impl QueryKind for Conversations {
    type Item = Conversation;
    type Id = ConversationId;

    const PATH: &'static str = CONVERSATIONS;
    const FIELD: &'static str = "Conversations";
    const ITEMS: Table<Conversation> = cached::CONVERSATIONS;
    const PAGES: Table<Vec<ConversationId>> = cached::CONVERSATION_PAGES;

    fn id(item: &Conversation) -> &ConversationId {
        item.id()
    }

    fn key(id: &ConversationId) -> &str {
        id.as_str()
    }

    fn labelled(item: &Conversation, label: &LabelId) -> bool {
        item.label(label).is_some()
    }
}
