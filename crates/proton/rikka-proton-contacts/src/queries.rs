use rikka_proton_api::{Page, Paging, RequestBuilder, offload};
use serde::Deserialize;

use crate::client::{CONTACTS, ContactsClient};
use crate::contact::Contact;
use crate::contact_email::ContactEmail;
use crate::decrypted_contact::DecryptedContact;
use crate::error::Result;
use crate::identifier::GroupId;

const CONTACT_PAGE: u32 = 1000;
const EXPORT_PAGE: u32 = 50;

#[derive(Deserialize)]
struct ContactsResponse {
    #[serde(rename = "Contacts", default)]
    contacts: Vec<Contact>,
    #[serde(rename = "Total", default)]
    total: Option<usize>,
}

#[derive(Deserialize)]
struct EmailsResponse {
    #[serde(rename = "ContactEmails", default)]
    emails: Vec<ContactEmail>,
    #[serde(rename = "Total", default)]
    total: Option<usize>,
}

#[must_use]
#[derive(Debug, Clone)]
pub struct ContactQuery {
    client: ContactsClient,
    group: Option<GroupId>,
    page_size: u32,
}

impl ContactQuery {
    pub(crate) fn new(client: ContactsClient) -> ContactQuery {
        ContactQuery {
            client,
            group: None,
            page_size: CONTACT_PAGE,
        }
    }

    pub fn in_group(mut self, group: &GroupId) -> ContactQuery {
        self.group = Some(group.clone());
        self
    }

    pub fn page_size(mut self, size: u32) -> ContactQuery {
        self.page_size = size.clamp(1, CONTACT_PAGE);
        self
    }

    pub async fn list(self) -> Result<Vec<Contact>> {
        let mut request = self.client.api().get(CONTACTS);
        if let Some(group) = &self.group {
            request = request.query(&[("LabelID", group.as_str())]);
        }
        let contacts: Vec<Contact> = request
            .pages(Paging::numbered(self.page_size))
            .collect(|response: ContactsResponse| {
                Page::new(response.contacts).total(response.total)
            })
            .await?;
        self.client.cache_contacts(&contacts);
        Ok(contacts)
    }
}

#[must_use]
#[derive(Debug, Clone)]
pub struct ContactEmailQuery {
    client: ContactsClient,
    email: Option<String>,
    group: Option<GroupId>,
    page_size: u32,
}

impl ContactEmailQuery {
    pub(crate) fn new(client: ContactsClient) -> ContactEmailQuery {
        ContactEmailQuery {
            client,
            email: None,
            group: None,
            page_size: CONTACT_PAGE,
        }
    }

    pub fn email(mut self, email: impl Into<String>) -> ContactEmailQuery {
        self.email = Some(email.into());
        self.group = None;
        self
    }

    pub fn in_group(mut self, group: &GroupId) -> ContactEmailQuery {
        self.group = Some(group.clone());
        self.email = None;
        self
    }

    pub fn page_size(mut self, size: u32) -> ContactEmailQuery {
        self.page_size = size.clamp(1, CONTACT_PAGE);
        self
    }

    pub async fn list(self) -> Result<Vec<ContactEmail>> {
        let emails: Vec<ContactEmail> = self
            .request()
            .pages(Paging::numbered(self.page_size))
            .collect(|response: EmailsResponse| Page::new(response.emails).total(response.total))
            .await?;
        self.client.cache_emails(&emails);
        Ok(emails)
    }

    pub(crate) async fn first(self) -> Result<Option<ContactEmail>> {
        let response: EmailsResponse = self
            .request()
            .query(&[("Page", "0"), ("PageSize", "1")])
            .send()
            .await?;
        let first = response.emails.into_iter().next();
        self.client.cache_emails(first.as_slice());
        Ok(first)
    }

    fn request(&self) -> RequestBuilder {
        let mut request = self.client.api().get(format!("{CONTACTS}/emails"));
        if let Some(email) = &self.email {
            request = request.query(&[("Email", email.as_str())]);
        }
        if let Some(group) = &self.group {
            request = request.query(&[("LabelID", group.as_str())]);
        }
        request
    }
}

#[must_use]
#[derive(Debug, Clone)]
pub struct ExportQuery {
    client: ContactsClient,
    group: Option<GroupId>,
}

impl ExportQuery {
    pub(crate) fn new(client: ContactsClient) -> ExportQuery {
        ExportQuery {
            client,
            group: None,
        }
    }

    pub fn in_group(mut self, group: &GroupId) -> ExportQuery {
        self.group = Some(group.clone());
        self
    }

    pub async fn contacts(self) -> Result<Vec<Contact>> {
        let mut request = self.client.api().get(format!("{CONTACTS}/export"));
        if let Some(group) = &self.group {
            request = request.query(&[("LabelID", group.as_str())]);
        }
        let contacts: Vec<Contact> = request
            .pages(Paging::numbered(EXPORT_PAGE))
            .collect(|response: ContactsResponse| {
                Page::new(response.contacts).total(response.total)
            })
            .await?;
        self.client.cache_contacts(&contacts);
        Ok(contacts)
    }

    pub async fn decrypt_all(self) -> Result<Vec<DecryptedContact>> {
        let client = self.client.clone();
        let contacts = self.contacts().await?;
        client.decrypt_contacts(contacts).await
    }

    pub async fn vcf(self) -> Result<String> {
        let contacts = self.decrypt_all().await?;
        Ok(offload(move || {
            let cards: Vec<_> = contacts
                .into_iter()
                .map(DecryptedContact::into_vcard)
                .collect();
            crate::vcard::write_vcards(&cards)
        })
        .await)
    }
}
