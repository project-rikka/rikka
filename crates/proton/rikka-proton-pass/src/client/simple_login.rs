use rikka_proton_api::{Page, Paging};
use rikka_proton_pgp::random_uuid;

use crate::api::ItemPage;
use crate::api::{
    AliasDomain, AliasSettings, AliasSyncStatus, CreatePendingAlias, CreatePendingAliasesRequest,
    DefaultDomainRequest, DefaultMailboxRequest, DeleteMailboxRequest, EnableSyncRequest,
    MailboxEmailRequest, PendingAlias, PendingAliasPage, UserMailbox, VerificationCodeRequest,
};
use crate::client::{LARGE_BATCH, PREFIX, PassClient};
use crate::content::{Item, ItemAlias, ItemContent, Metadata};
use crate::crypto::RotatedKey;
use crate::error::Result;
use crate::identifier::ShareId;
use crate::new_item::NewItem;
use crate::pass_item::PassItem;

impl PassClient {
    pub async fn alias_sync_status(&self) -> Result<AliasSyncStatus> {
        let envelope: Option<AliasSyncStatus> = self
            .api()
            .get(format!("{PREFIX}/alias_sync/status"))
            .send_field_or_default("SyncStatus")
            .await?;

        Ok(envelope.unwrap_or_default())
    }

    pub async fn enable_alias_sync(&self, default_share: Option<&ShareId>) -> Result<()> {
        self.api()
            .post(format!("{PREFIX}/alias_sync/sync"))
            .json(&EnableSyncRequest { default_share })
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn pending_aliases(&self) -> Result<Vec<PendingAlias>> {
        Ok(self
            .api()
            .get(format!("{PREFIX}/alias_sync/pending"))
            .pages(Paging::since())
            .field("PendingAliases")
            .collect(|page: PendingAliasPage| Page::new(page.aliases).next(page.last_token))
            .await?)
    }

    pub async fn create_pending_aliases(
        &self,
        share: &ShareId,
        aliases: &[PendingAlias],
    ) -> Result<Vec<PassItem>> {
        if aliases.is_empty() {
            return Ok(Vec::new());
        }

        let parent = RotatedKey::from(self.latest_share_key(share).await?);
        let mut revisions = Vec::with_capacity(aliases.len());

        for batch in aliases.chunks(LARGE_BATCH) {
            let items = batch
                .iter()
                .map(|alias| {
                    let content = Item::new(
                        Metadata {
                            name: alias.email().to_owned(),
                            note: String::new(),
                            item_uuid: random_uuid(),
                            custom_icon: None,
                        },
                        ItemContent::Alias(ItemAlias {}),
                    );
                    let item =
                        crate::crypto::item::create_request(&parent, &NewItem::new(content))?;
                    Ok(CreatePendingAlias {
                        id: alias.id().clone(),
                        item,
                    })
                })
                .collect::<Result<Vec<CreatePendingAlias>>>()?;

            let envelope: ItemPage = self
                .api()
                .post(format!("{PREFIX}/alias_sync/share/{share}/create"))
                .json(&CreatePendingAliasesRequest { items })
                .send_field("Revisions")
                .await?;
            revisions.extend(envelope.revisions);
        }

        self.decrypt_items(share, revisions).await
    }

    pub async fn alias_settings(&self) -> Result<AliasSettings> {
        let envelope: Option<AliasSettings> = self
            .api()
            .get(format!("{PREFIX}/user/alias/settings"))
            .send_field_or_default("Settings")
            .await?;

        Ok(envelope.unwrap_or_default())
    }

    pub async fn set_default_alias_domain(&self, domain: Option<&str>) -> Result<AliasSettings> {
        let envelope: Option<AliasSettings> = self
            .api()
            .put(format!("{PREFIX}/user/alias/settings/default_alias_domain"))
            .json(&DefaultDomainRequest { domain })
            .send_field_or_default("Settings")
            .await?;

        Ok(envelope.unwrap_or_default())
    }

    pub async fn set_default_alias_mailbox(&self, mailbox: i64) -> Result<AliasSettings> {
        let envelope: Option<AliasSettings> = self
            .api()
            .put(format!("{PREFIX}/user/alias/settings/default_mailbox_id"))
            .json(&DefaultMailboxRequest { mailbox })
            .send_field_or_default("Settings")
            .await?;

        Ok(envelope.unwrap_or_default())
    }

    pub async fn alias_domains(&self) -> Result<Vec<AliasDomain>> {
        let envelope: Vec<AliasDomain> = self
            .api()
            .get(format!("{PREFIX}/user/alias/domain"))
            .send_field_or_default("Domains")
            .await?;

        Ok(envelope)
    }

    pub async fn alias_mailboxes(&self) -> Result<Vec<UserMailbox>> {
        let envelope: Vec<UserMailbox> = self
            .api()
            .get(format!("{PREFIX}/user/alias/mailbox"))
            .send_field_or_default("Mailboxes")
            .await?;

        Ok(envelope)
    }

    pub async fn create_alias_mailbox(&self, email: &str) -> Result<UserMailbox> {
        let envelope: UserMailbox = self
            .api()
            .post(format!("{PREFIX}/user/alias/mailbox"))
            .json(&MailboxEmailRequest { email })
            .send_field("Mailbox")
            .await?;

        Ok(envelope)
    }

    pub async fn verify_alias_mailbox(&self, mailbox: i64, code: &str) -> Result<UserMailbox> {
        let envelope: UserMailbox = self
            .api()
            .post(format!("{PREFIX}/user/alias/mailbox/{mailbox}/verify"))
            .json(&VerificationCodeRequest { code })
            .send_field("Mailbox")
            .await?;

        Ok(envelope)
    }

    pub async fn resend_alias_mailbox_code(&self, mailbox: i64) -> Result<UserMailbox> {
        let envelope: UserMailbox = self
            .api()
            .get(format!("{PREFIX}/user/alias/mailbox/{mailbox}/verify"))
            .send_field("Mailbox")
            .await?;

        Ok(envelope)
    }

    pub async fn change_alias_mailbox_email(
        &self,
        mailbox: i64,
        email: &str,
    ) -> Result<UserMailbox> {
        let envelope: UserMailbox = self
            .api()
            .put(format!("{PREFIX}/user/alias/mailbox/{mailbox}/email"))
            .json(&MailboxEmailRequest { email })
            .send_field("Mailbox")
            .await?;

        Ok(envelope)
    }

    pub async fn cancel_alias_mailbox_email_change(&self, mailbox: i64) -> Result<()> {
        self.api()
            .delete(format!("{PREFIX}/user/alias/mailbox/{mailbox}/email"))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn delete_alias_mailbox(&self, mailbox: i64, transfer_to: Option<i64>) -> Result<()> {
        self.api()
            .delete(format!("{PREFIX}/user/alias/mailbox/{mailbox}"))
            .json(&DeleteMailboxRequest { transfer_to })
            .send_unit()
            .await?;

        Ok(())
    }
}
