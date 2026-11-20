use rikka_proton_api::{Outcome, flag, parallel_map_each};
use serde::Serialize;

use crate::card::ContactCard;
use crate::client::{CONTACTS, ContactsClient};
use crate::error::Result;
use crate::outcome::ContactReply;
use crate::sealing::seal_vcard;
use crate::vcard::VCard;

const BATCH: usize = 10;

#[must_use]
#[derive(Debug, Clone)]
pub struct ContactCreation {
    client: ContactsClient,
    cards: Vec<VCard>,
    overwrite: bool,
    import: bool,
    groups_from_categories: bool,
}

#[derive(Serialize)]
struct SealedContact {
    #[serde(rename = "Cards")]
    cards: Vec<ContactCard>,
}

#[derive(Serialize)]
struct CreationBody<'a> {
    #[serde(rename = "Contacts")]
    contacts: &'a [SealedContact],
    #[serde(rename = "Overwrite", with = "flag")]
    overwrite: bool,
    #[serde(rename = "Import", with = "flag")]
    import: bool,
    #[serde(rename = "Labels", with = "flag")]
    labels: bool,
}

impl ContactCreation {
    pub(crate) fn new(client: ContactsClient, cards: Vec<VCard>) -> ContactCreation {
        ContactCreation {
            client,
            cards,
            overwrite: false,
            import: false,
            groups_from_categories: false,
        }
    }

    pub fn overwrite(mut self, overwrite: bool) -> ContactCreation {
        self.overwrite = overwrite;
        self
    }

    pub fn as_import(mut self, import: bool) -> ContactCreation {
        self.import = import;
        self
    }

    pub fn groups_from_categories(mut self, include: bool) -> ContactCreation {
        self.groups_from_categories = include;
        self
    }

    pub async fn execute(self) -> Result<Vec<Outcome<String, ContactReply>>> {
        let key = self.client.signing_key()?;
        let pgp = *self.client.pgp();
        let sealed: Vec<Result<SealedContact>> = parallel_map_each(self.cards, move |card| {
            Ok(SealedContact {
                cards: seal_vcard(&pgp, &card, &key)?,
            })
        })
        .await;
        let sealed = sealed.into_iter().collect::<Result<Vec<_>>>()?;

        let mut outcomes = Vec::with_capacity(sealed.len());
        for (batch_index, contacts) in sealed.chunks(BATCH).enumerate() {
            let body = CreationBody {
                contacts,
                overwrite: self.overwrite,
                import: self.import,
                labels: self.groups_from_categories,
            };
            let response: Vec<Outcome<String, ContactReply>> = self
                .client
                .api()
                .post(CONTACTS)
                .json(&body)
                .send_field_or_default("Responses")
                .await?;
            outcomes.extend(
                response
                    .into_iter()
                    .map(|outcome| outcome.offset_by(batch_index * BATCH)),
            );
        }

        Ok(outcomes)
    }
}
