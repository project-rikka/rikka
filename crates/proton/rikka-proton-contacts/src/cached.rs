use rikka_proton_cache::{Batch, Cache, Table};

use crate::client::ContactsClient;
use crate::contact::Contact;
use crate::contact_email::ContactEmail;
use crate::event::{ContactBook, ContactEvents, ContactModel};
use crate::group::ContactGroup;
use crate::identifier::{ContactEmailId, ContactId, GroupId};

const CONTACTS: Table<Contact> = Table::new("contacts.contacts");
const EMAILS: Table<ContactEmail> = Table::new("contacts.emails");
const GROUPS: Table<ContactGroup> = Table::new("contacts.groups");

impl ContactsClient {
    #[must_use]
    pub fn with_cache(mut self, cache: Cache) -> ContactsClient {
        self.account = self.account.with_cache(cache);
        self
    }

    #[must_use]
    pub fn cache(&self) -> Option<&Cache> {
        self.account.cache()
    }

    #[must_use]
    pub fn cached_contact(&self, id: &ContactId) -> Option<Contact> {
        self.cache()?.get(&CONTACTS, id.as_str())
    }

    #[must_use]
    pub fn cached_book(&self) -> Option<ContactBook> {
        let cache = self.cache()?;
        let contacts = cache.values(&CONTACTS);
        let emails = cache.values(&EMAILS);
        let groups = cache.values(&GROUPS);
        if contacts.is_empty() && emails.is_empty() && groups.is_empty() {
            return None;
        }
        Some(ContactBook::from_parts(contacts, emails, groups))
    }

    pub fn apply_events(&self, events: &ContactEvents) {
        let Some(cache) = self.cache() else {
            return;
        };
        let mut writer = CacheWriter {
            cache,
            batch: cache.batch(),
        };
        events.apply(&mut writer);
        writer.batch.commit();
    }

    pub fn forget_cached(&self) {
        if let Some(cache) = self.cache() {
            let mut batch = cache.batch();
            batch.clear(&CONTACTS).clear(&EMAILS).clear(&GROUPS);
            batch.commit();
        }
    }

    pub(crate) fn cache_contacts(&self, contacts: &[Contact]) {
        let Some(cache) = self.cache() else {
            return;
        };
        let mut batch = cache.batch();
        for contact in contacts {
            let would_drop_cards =
                !contact.has_cards() && cache.contains(&CONTACTS, contact.id().as_str());
            if !would_drop_cards {
                batch.put(&CONTACTS, contact.id().as_str(), contact.clone());
            }
        }
        batch.commit();
    }

    pub(crate) fn cache_emails(&self, emails: &[ContactEmail]) {
        let Some(cache) = self.cache() else {
            return;
        };
        let mut batch = cache.batch();
        for email in emails {
            batch.put(&EMAILS, email.id().as_str(), email.clone());
        }
        batch.commit();
    }

    pub(crate) fn cache_groups(&self, groups: &[ContactGroup]) {
        let Some(cache) = self.cache() else {
            return;
        };
        cache.replace(
            &GROUPS,
            groups
                .iter()
                .map(|group| (group.id().as_str().to_owned(), group.clone())),
        );
    }
}

struct CacheWriter<'a> {
    cache: &'a Cache,
    batch: Batch,
}

impl ContactModel for CacheWriter<'_> {
    fn put_contact(&mut self, contact: Contact) {
        let id = contact.id().as_str().to_owned();
        self.batch.put(&CONTACTS, &id, contact);
    }

    fn remove_contact(&mut self, id: &ContactId) {
        self.batch.remove(&CONTACTS, id.as_str());
        for (email_id, email) in self.cache.entries(&EMAILS) {
            if email.contact_id() == id {
                self.batch.remove(&EMAILS, &email_id);
            }
        }
    }

    fn put_email(&mut self, email: ContactEmail) {
        let id = email.id().as_str().to_owned();
        self.batch.put(&EMAILS, &id, email);
    }

    fn remove_email(&mut self, id: &ContactEmailId) {
        self.batch.remove(&EMAILS, id.as_str());
    }

    fn put_group(&mut self, group: ContactGroup) {
        let id = group.id().as_str().to_owned();
        self.batch.put(&GROUPS, &id, group);
    }

    fn remove_group(&mut self, id: &GroupId) {
        self.batch.remove(&GROUPS, id.as_str());
    }

    fn reload(&mut self) {
        self.batch.clear(&CONTACTS).clear(&EMAILS).clear(&GROUPS);
    }
}
