use crate::ItemRevision;
use crate::api::{
    AliasDetails, AliasMailboxesRequest, AliasNameRequest, AliasNoteRequest, AliasOptions,
    AliasStatusRequest, CreateAliasRequest,
};
use crate::client::item::decrypt_revision;
use crate::client::{PREFIX, PassClient};
use crate::crypto::RotatedKey;
use crate::error::Result;
use crate::identifier::{ItemId, ShareId};
use crate::new_alias::NewAlias;
use crate::pass_item::PassItem;

impl PassClient {
    pub async fn alias_options(&self, share: &ShareId) -> Result<AliasOptions> {
        let envelope: AliasOptions = self
            .api()
            .get(format!("{PREFIX}/share/{share}/alias/options"))
            .send_field("Options")
            .await?;

        Ok(envelope)
    }

    pub async fn create_alias(&self, share: &ShareId, alias: NewAlias) -> Result<PassItem> {
        let NewAlias {
            prefix,
            signed_suffix,
            mailboxes,
            name,
            item,
        } = alias;

        let parent = RotatedKey::from(self.latest_share_key(share).await?);
        let request = CreateAliasRequest {
            prefix,
            signed_suffix,
            mailbox_ids: mailboxes,
            alias_name: name,
            item: crate::crypto::item::create_request(&parent, &item)?,
        };

        let envelope: ItemRevision = self
            .api()
            .post(format!("{PREFIX}/share/{share}/alias/custom"))
            .json(&request)
            .send_field("Item")
            .await?;

        decrypt_revision(share, &parent.key, envelope)
    }

    pub async fn alias_details(&self, share: &ShareId, item: &ItemId) -> Result<AliasDetails> {
        let envelope: AliasDetails = self
            .api()
            .get(format!("{PREFIX}/share/{share}/alias/{item}"))
            .send_field("Alias")
            .await?;

        Ok(envelope)
    }

    pub async fn set_alias_mailboxes(
        &self,
        share: &ShareId,
        item: &ItemId,
        mailboxes: impl IntoIterator<Item = i64>,
    ) -> Result<AliasDetails> {
        let request = AliasMailboxesRequest {
            mailbox_ids: mailboxes.into_iter().collect(),
        };

        let envelope: AliasDetails = self
            .api()
            .post(format!("{PREFIX}/share/{share}/alias/{item}/mailbox"))
            .json(&request)
            .send_field("Alias")
            .await?;

        Ok(envelope)
    }

    pub async fn set_alias_enabled(
        &self,
        share: &ShareId,
        item: &ItemId,
        enabled: bool,
    ) -> Result<()> {
        self.api()
            .put(format!("{PREFIX}/share/{share}/alias/{item}/status"))
            .json(&AliasStatusRequest { enable: enabled })
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn set_alias_name(
        &self,
        share: &ShareId,
        item: &ItemId,
        name: impl Into<String>,
    ) -> Result<()> {
        self.api()
            .put(format!("{PREFIX}/share/{share}/alias/{item}/name"))
            .json(&AliasNameRequest { name: name.into() })
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn set_alias_note(
        &self,
        share: &ShareId,
        item: &ItemId,
        note: impl Into<String>,
    ) -> Result<()> {
        self.api()
            .put(format!("{PREFIX}/share/{share}/alias/{item}/note"))
            .json(&AliasNoteRequest { note: note.into() })
            .send_unit()
            .await?;

        Ok(())
    }
}
