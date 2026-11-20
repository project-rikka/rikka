use rikka_proton_api::{Page, Paging};

use crate::api::{
    AliasContact, AliasContactsEnvelope, BlockAliasContactRequest, CreateAliasContactRequest,
};
use crate::client::{PREFIX, PassClient};
use crate::error::Result;
use crate::identifier::{ItemId, ShareId};

impl PassClient {
    pub async fn alias_contacts(
        &self,
        share: &ShareId,
        item: &ItemId,
    ) -> Result<Vec<AliasContact>> {
        Ok(self
            .api()
            .get(format!("{PREFIX}/share/{share}/alias/{item}/contact"))
            .pages(Paging::since())
            .collect(|envelope: AliasContactsEnvelope| {
                Page::new(envelope.contacts).next(envelope.last_id.filter(|last| *last > 0))
            })
            .await?)
    }

    pub async fn alias_contact(
        &self,
        share: &ShareId,
        item: &ItemId,
        contact: i64,
    ) -> Result<AliasContact> {
        let envelope: AliasContact = self
            .api()
            .get(format!(
                "{PREFIX}/share/{share}/alias/{item}/contact/{contact}"
            ))
            .send_field("Contact")
            .await?;

        Ok(envelope)
    }

    pub async fn create_alias_contact(
        &self,
        share: &ShareId,
        item: &ItemId,
        email: &str,
        name: Option<&str>,
    ) -> Result<AliasContact> {
        let envelope: AliasContact = self
            .api()
            .post(format!("{PREFIX}/share/{share}/alias/{item}/contact"))
            .json(&CreateAliasContactRequest { email, name })
            .send_field("Contact")
            .await?;

        Ok(envelope)
    }

    pub async fn delete_alias_contact(
        &self,
        share: &ShareId,
        item: &ItemId,
        contact: i64,
    ) -> Result<()> {
        self.api()
            .delete(format!(
                "{PREFIX}/share/{share}/alias/{item}/contact/{contact}"
            ))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn set_alias_contact_blocked(
        &self,
        share: &ShareId,
        item: &ItemId,
        contact: i64,
        blocked: bool,
    ) -> Result<AliasContact> {
        let envelope: AliasContact = self
            .api()
            .put(format!(
                "{PREFIX}/share/{share}/alias/{item}/contact/{contact}/blocked"
            ))
            .json(&BlockAliasContactRequest { blocked })
            .send_field("Contact")
            .await?;

        Ok(envelope)
    }
}
