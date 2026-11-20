use std::collections::BTreeMap;

use rikka_proton_account::{EventId, Refresh};
use rikka_proton_api::change;
use serde::Deserialize;

use crate::contact::Contact;
use crate::contact_email::ContactEmail;
use crate::group::ContactGroup;
use crate::identifier::{ContactEmailId, ContactId, GroupId};

change!(ContactChange, ContactId, contact, Contact, "Contact");
change!(
    ContactEmailChange,
    ContactEmailId,
    contact_email,
    ContactEmail,
    "ContactEmail"
);
change!(GroupChange, GroupId, group, ContactGroup, "Label");

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ContactEvents {
    #[serde(rename = "EventID", default)]
    id: EventId,
    #[serde(rename = "Refresh", default)]
    refresh: Refresh,
    #[serde(
        rename = "More",
        default,
        deserialize_with = "rikka_proton_api::flag::deserialize"
    )]
    more: bool,
    #[serde(rename = "Contacts", default)]
    contacts: Vec<ContactChange>,
    #[serde(rename = "ContactEmails", default)]
    contact_emails: Vec<ContactEmailChange>,
    #[serde(rename = "Labels", default)]
    labels: Vec<GroupChange>,
}

impl ContactEvents {
    #[must_use]
    pub fn id(&self) -> &EventId {
        &self.id
    }

    #[must_use]
    pub fn has_more(&self) -> bool {
        self.more
    }

    #[must_use]
    pub fn needs_reload(&self) -> bool {
        self.refresh.includes_contacts()
    }

    #[must_use]
    pub fn contacts(&self) -> &[ContactChange] {
        &self.contacts
    }

    #[must_use]
    pub fn contact_emails(&self) -> &[ContactEmailChange] {
        &self.contact_emails
    }

    pub fn groups(&self) -> impl Iterator<Item = &GroupChange> {
        self.labels.iter().filter(|change| {
            change.is_deleted() || change.group().is_some_and(ContactGroup::is_contact_group)
        })
    }

    pub fn apply(&self, model: &mut impl ContactModel) {
        if self.needs_reload() {
            model.reload();
            return;
        }
        for change in &self.contacts {
            match change.contact() {
                Some(contact) if !change.is_deleted() => model.put_contact(contact.clone()),
                _ => model.remove_contact(change.id()),
            }
        }
        for change in &self.contact_emails {
            match change.contact_email() {
                Some(email) if !change.is_deleted() => model.put_email(email.clone()),
                _ => model.remove_email(change.id()),
            }
        }
        for change in self.groups() {
            match change.group() {
                Some(group) if !change.is_deleted() => model.put_group(group.clone()),
                _ => model.remove_group(change.id()),
            }
        }
    }
}

pub trait ContactModel {
    fn put_contact(&mut self, contact: Contact);
    fn remove_contact(&mut self, id: &ContactId);
    fn put_email(&mut self, email: ContactEmail);
    fn remove_email(&mut self, id: &ContactEmailId);
    fn put_group(&mut self, group: ContactGroup);
    fn remove_group(&mut self, id: &GroupId);
    fn reload(&mut self);
}

#[derive(Debug, Clone, Default)]
pub struct ContactBook {
    contacts: BTreeMap<ContactId, Contact>,
    emails: BTreeMap<ContactEmailId, ContactEmail>,
    groups: BTreeMap<GroupId, ContactGroup>,
    stale: bool,
}

impl ContactBook {
    #[must_use]
    pub fn new() -> ContactBook {
        ContactBook::default()
    }

    #[must_use]
    pub fn from_parts(
        contacts: Vec<Contact>,
        emails: Vec<ContactEmail>,
        groups: Vec<ContactGroup>,
    ) -> ContactBook {
        ContactBook {
            contacts: contacts
                .into_iter()
                .map(|contact| (contact.id().clone(), contact))
                .collect(),
            emails: emails
                .into_iter()
                .map(|email| (email.id().clone(), email))
                .collect(),
            groups: groups
                .into_iter()
                .map(|group| (group.id().clone(), group))
                .collect(),
            stale: false,
        }
    }

    #[must_use]
    pub fn contact(&self, id: &ContactId) -> Option<&Contact> {
        self.contacts.get(id)
    }

    pub fn contacts(&self) -> impl Iterator<Item = &Contact> {
        self.contacts.values()
    }

    #[must_use]
    pub fn email(&self, id: &ContactEmailId) -> Option<&ContactEmail> {
        self.emails.get(id)
    }

    pub fn emails(&self) -> impl Iterator<Item = &ContactEmail> {
        self.emails.values()
    }

    pub fn emails_for<'a>(
        &'a self,
        contact: &'a ContactId,
    ) -> impl Iterator<Item = &'a ContactEmail> + 'a {
        self.emails
            .values()
            .filter(move |email| email.contact_id() == contact)
    }

    #[must_use]
    pub fn group(&self, id: &GroupId) -> Option<&ContactGroup> {
        self.groups.get(id)
    }

    pub fn groups(&self) -> impl Iterator<Item = &ContactGroup> {
        self.groups.values()
    }

    #[must_use]
    pub fn is_stale(&self) -> bool {
        self.stale
    }
}

impl ContactModel for ContactBook {
    fn put_contact(&mut self, contact: Contact) {
        self.contacts.insert(contact.id().clone(), contact);
    }

    fn remove_contact(&mut self, id: &ContactId) {
        self.contacts.remove(id);
        self.emails.retain(|_, email| email.contact_id() != id);
    }

    fn put_email(&mut self, email: ContactEmail) {
        self.emails.insert(email.id().clone(), email);
    }

    fn remove_email(&mut self, id: &ContactEmailId) {
        self.emails.remove(id);
    }

    fn put_group(&mut self, group: ContactGroup) {
        self.groups.insert(group.id().clone(), group);
    }

    fn remove_group(&mut self, id: &GroupId) {
        self.groups.remove(id);
    }

    fn reload(&mut self) {
        self.contacts.clear();
        self.emails.clear();
        self.groups.clear();
        self.stale = true;
    }
}
