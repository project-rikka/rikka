use rikka_proton_api::EventAction;
use rikka_proton_cache::{Cache, Table};

use crate::conversation::Conversation;
use crate::count::LabelCount;
use crate::event::Event;
use crate::identifier::{ConversationId, MessageId};
use crate::label::{Label, LabelType};
use crate::message::MessageMetadata;

pub(crate) const LABELS: Table<Vec<Label>> = Table::new("mail.labels");
const KEPT_ROWS: usize = 20_000;
const KEPT_PAGES: usize = 1_024;

pub(crate) const MESSAGES: Table<MessageMetadata> = Table::bounded("mail.messages", KEPT_ROWS);
pub(crate) const CONVERSATIONS: Table<Conversation> =
    Table::bounded("mail.conversations", KEPT_ROWS);
pub(crate) const MESSAGE_PAGES: Table<Vec<MessageId>> =
    Table::bounded("mail.message-pages", KEPT_PAGES);
pub(crate) const CONVERSATION_PAGES: Table<Vec<ConversationId>> =
    Table::bounded("mail.conversation-pages", KEPT_PAGES);
pub(crate) const COUNTS: Table<Vec<LabelCount>> = Table::new("mail.counts");

pub(crate) const MESSAGE_COUNTS: &str = crate::client::MESSAGES;
pub(crate) const CONVERSATION_COUNTS: &str = crate::client::CONVERSATIONS;

pub(crate) fn labels(cache: &Cache, kind: LabelType) -> Option<Vec<Label>> {
    cache.get(&LABELS, &kind.number().to_string())
}

pub(crate) fn cache_labels(cache: &Cache, kind: LabelType, labels: &[Label]) {
    cache.put(&LABELS, &kind.number().to_string(), labels.to_vec());
}

pub(crate) fn apply(cache: &Cache, event: &Event) {
    if event.refresh().includes_mail() {
        clear(cache);
        return;
    }

    let mut batch = cache.batch();
    for change in event.messages() {
        match (change.action(), change.message()) {
            (EventAction::Delete, _) | (_, None) => {
                batch.remove(&MESSAGES, change.id().as_str());
            }
            (_, Some(message)) => {
                batch.put(&MESSAGES, change.id().as_str(), message.clone());
            }
        }
    }
    for change in event.conversations() {
        match (change.action(), change.conversation()) {
            (EventAction::Delete, _) | (_, None) => {
                batch.remove(&CONVERSATIONS, change.id().as_str());
            }
            (_, Some(conversation)) => {
                batch.put(&CONVERSATIONS, change.id().as_str(), conversation.clone());
            }
        }
    }
    for kind in LabelType::all() {
        let Some(mut cached_labels) = labels(cache, *kind) else {
            continue;
        };
        let mut changed = false;
        for change in event.labels() {
            let before = cached_labels.len();
            cached_labels.retain(|label| label.id() != change.id());
            changed |= cached_labels.len() != before;
            if let Some(label) = change
                .label()
                .filter(|label| change.action() != EventAction::Delete && label.kind() == *kind)
            {
                cached_labels.push(label.clone());
                changed = true;
            }
        }
        if changed {
            cached_labels.sort_by_key(Label::order);
            batch.put(&LABELS, &kind.number().to_string(), cached_labels);
        }
    }
    for (collection, counts) in [
        (MESSAGE_COUNTS, event.message_counts()),
        (CONVERSATION_COUNTS, event.conversation_counts()),
    ] {
        if let Some(counts) = counts {
            batch.put(&COUNTS, collection, counts.to_vec());
        }
    }
    batch.commit();
}

pub(crate) fn clear(cache: &Cache) {
    let mut batch = cache.batch();
    batch
        .clear(&LABELS)
        .clear(&MESSAGES)
        .clear(&CONVERSATIONS)
        .clear(&MESSAGE_PAGES)
        .clear(&CONVERSATION_PAGES)
        .clear(&COUNTS);
    batch.commit();
}
