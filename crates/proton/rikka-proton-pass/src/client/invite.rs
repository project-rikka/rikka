use std::collections::HashMap;
use std::sync::Arc;

use rikka_proton_account::UserKeys;
use rikka_proton_pgp::PublicKey;

use crate::api::{
    CheckAddressesRequest, CreateInviteRequest, CreateInvitesRequest, CreateNewUserInviteRequest,
    CreateNewUserInvitesRequest, InviteKeysRequest, InviteSuggestion, NewUserInvite,
    OrganizationRecommendations, ShareInvites, ShareResponse, ShareTarget, UserInviteResponse,
};
use crate::client::public_keys::select_encryption_key;
use crate::client::{PREFIX, PassClient, SMALL_BATCH};
use crate::crypto::{RotatedKey, ShareKeys};
use crate::error::{Error, Result};
use crate::identifier::{InviteId, InviteToken, ItemId, NewUserInviteId, ShareId};
use crate::invitation::Invitation;
use crate::invited::Invited;
use crate::pending_invite::PendingInvite;
use crate::share::Share;

enum PreparedInvite {
    Existing(CreateInviteRequest),
    NewUser(CreateNewUserInviteRequest),
}

struct InviterContext {
    share: ShareResponse,
    keys: UserKeys,
    share_keys: ShareKeys,
    item_keys: HashMap<ItemId, Vec<RotatedKey>>,
}

impl PassClient {
    pub async fn invite(&self, share: &ShareId, invitation: Invitation) -> Result<Invited> {
        let mut inviter = self.inviter_context(share).await?;
        let email = invitation.email().to_owned();

        match self.prepare_invite(&mut inviter, invitation).await? {
            PreparedInvite::Existing(request) => {
                self.api()
                    .post(format!("{PREFIX}/share/{share}/invite"))
                    .json(&request)
                    .send_unit()
                    .await?;
                Ok(Invited::new(email, false))
            }
            PreparedInvite::NewUser(request) => {
                self.api()
                    .post(format!("{PREFIX}/share/{share}/invite/new_user"))
                    .json(&request)
                    .send_unit()
                    .await?;
                Ok(Invited::new(email, true))
            }
        }
    }

    pub async fn invite_all(
        &self,
        share: &ShareId,
        invitations: impl IntoIterator<Item = Invitation>,
    ) -> Result<Vec<Invited>> {
        let mut inviter = self.inviter_context(share).await?;
        let mut existing_users = Vec::new();
        let mut new_users = Vec::new();
        let mut invited = Vec::new();

        for invitation in invitations {
            let email = invitation.email().to_owned();
            match self.prepare_invite(&mut inviter, invitation).await? {
                PreparedInvite::Existing(request) => {
                    existing_users.push(request);
                    invited.push(Invited::new(email, false));
                }
                PreparedInvite::NewUser(request) => {
                    new_users.push(request);
                    invited.push(Invited::new(email, true));
                }
            }
        }

        for batch in existing_users.chunks(SMALL_BATCH) {
            self.api()
                .post(format!("{PREFIX}/share/{share}/invite/batch"))
                .json(&CreateInvitesRequest {
                    invites: batch.to_vec(),
                })
                .send_unit()
                .await?;
        }
        for batch in new_users.chunks(SMALL_BATCH) {
            self.api()
                .post(format!("{PREFIX}/share/{share}/invite/new_user/batch"))
                .json(&CreateNewUserInvitesRequest {
                    invites: batch.to_vec(),
                })
                .send_unit()
                .await?;
        }

        Ok(invited)
    }

    pub async fn share_invites(&self, share: &ShareId) -> Result<ShareInvites> {
        Ok(self
            .api()
            .get(format!("{PREFIX}/share/{share}/invite"))
            .send()
            .await?)
    }

    pub async fn resend_invite(&self, share: &ShareId, invite: &InviteId) -> Result<()> {
        self.api()
            .post(format!("{PREFIX}/share/{share}/invite/{invite}/reminder"))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn delete_invite(&self, share: &ShareId, invite: &InviteId) -> Result<()> {
        self.api()
            .delete(format!("{PREFIX}/share/{share}/invite/{invite}"))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn delete_new_user_invite(
        &self,
        share: &ShareId,
        invite: &NewUserInviteId,
    ) -> Result<()> {
        self.api()
            .delete(format!("{PREFIX}/share/{share}/invite/new_user/{invite}"))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn confirm_new_user_invite(
        &self,
        share: &ShareId,
        invite: &NewUserInvite,
    ) -> Result<()> {
        let inviter = self.inviter_context(share).await?;
        let latest = inviter
            .share_keys
            .latest()
            .ok_or_else(|| Error::NoShareKey {
                share: share.clone(),
            })?
            .key()
            .clone();
        let invitee = self.encryption_key(invite.invited_email()).await?;

        let (pgp, signer) = (*self.pgp(), inviter.keys.clone());
        let email = invite.invited_email().to_owned();
        let signature = invite.signature().to_owned();
        let keys = rotation_keys(&inviter.share_keys);

        let sealed = rikka_proton_api::offload(move || {
            crate::crypto::invite::verify_new_user_signature(
                &pgp, &signer, &email, &latest, &signature,
            )?;
            crate::crypto::invite::encrypt_for_invitee(&pgp, signer.signer(), &invitee, &keys)
        })
        .await?;

        let id = invite.id();
        self.api()
            .post(format!("{PREFIX}/share/{share}/invite/new_user/{id}/keys"))
            .json(&InviteKeysRequest { keys: sealed })
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn invite_suggestions(
        &self,
        share: &ShareId,
        contains: Option<&str>,
    ) -> Result<Vec<InviteSuggestion>> {
        let envelope: Vec<InviteSuggestion> = self
            .api()
            .get(format!(
                "{PREFIX}/share/{share}/invite/recommended_emails/suggested"
            ))
            .query(&present(&[("Contains", contains)]))
            .send_field_or_default("Suggested")
            .await?;

        Ok(envelope)
    }

    pub async fn organization_invite_recommendations(
        &self,
        share: &ShareId,
        contains: Option<&str>,
        since: Option<&str>,
    ) -> Result<OrganizationRecommendations> {
        let envelope: OrganizationRecommendations = self
            .api()
            .get(format!(
                "{PREFIX}/share/{share}/invite/recommended_emails/organization"
            ))
            .query(&present(&[("Contains", contains), ("PlanSince", since)]))
            .query(&[("PlanPageSize", crate::client::PAGE_SIZE)])
            .send_field("Recommendation")
            .await?;

        Ok(envelope)
    }

    pub async fn invitable_addresses(
        &self,
        share: &ShareId,
        emails: &[String],
    ) -> Result<Vec<String>> {
        let mut allowed = Vec::new();
        for batch in emails.chunks(SMALL_BATCH) {
            let envelope: Vec<String> = self
                .api()
                .post(format!("{PREFIX}/share/{share}/invite/check_address"))
                .json(&CheckAddressesRequest { emails: batch })
                .send_field_or_default("Emails")
                .await?;
            allowed.extend(envelope);
        }

        Ok(allowed)
    }

    pub async fn invites(&self) -> Result<Vec<PendingInvite>> {
        let invites: Vec<UserInviteResponse> = self
            .api()
            .get(format!("{PREFIX}/invite"))
            .send_field_or_default("Invites")
            .await?;

        let mut inviters: HashMap<String, Vec<PublicKey>> = HashMap::new();
        let mut decrypted = Vec::with_capacity(invites.len());

        for invite in invites {
            let Some(data) = invite.vault_data() else {
                decrypted.push(PendingInvite::new(invite, None));
                continue;
            };
            let Some(key) = invite
                .keys()
                .iter()
                .find(|key| key.rotation() == data.content_key_rotation())
                .cloned()
            else {
                tracing::warn!(
                    token = %invite.token(),
                    "vault invite has no content key"
                );
                continue;
            };

            let invited = match self.address_keys(invite.invited_address_id()) {
                Ok(keys) => keys.clone(),
                Err(error) => {
                    tracing::warn!(
                        token = %invite.token(),
                        %error,
                        "vault invite names locked address"
                    );
                    continue;
                }
            };
            let inviter_keys = match inviters.get(invite.inviter_email()) {
                Some(keys) => keys.clone(),
                None => match self.verifying_keys(invite.inviter_email()).await {
                    Ok(keys) => {
                        inviters.insert(invite.inviter_email().to_owned(), keys.clone());
                        keys
                    }
                    Err(error @ Error::NoPublicKey { .. }) => {
                        tracing::warn!(
                            token = %invite.token(),
                            %error,
                            "vault invite inviter has no keys"
                        );
                        continue;
                    }
                    Err(error) => return Err(error),
                },
            };

            let pgp = *self.pgp();
            let (vault, invite) = rikka_proton_api::offload(move || {
                let vault = invite.vault_data().map(|data| {
                    crate::crypto::invite::decrypt_invite_key(
                        &pgp,
                        &invited,
                        invite.inviter_email(),
                        &inviter_keys,
                        &key,
                    )
                    .and_then(|decrypted| {
                        crate::crypto::invite::decrypt_vault(&decrypted, data.content())
                    })
                });
                (vault, invite)
            })
            .await;

            match vault {
                Some(Ok(vault)) => decrypted.push(PendingInvite::new(invite, Some(vault))),
                Some(Err(error)) => {
                    tracing::warn!(
                        token = %invite.token(),
                        %error,
                        "failed to open vault invite"
                    );
                }
                None => decrypted.push(PendingInvite::new(invite, None)),
            }
        }

        Ok(decrypted)
    }

    pub async fn accept_invite(&self, invite: &PendingInvite) -> Result<Share> {
        let response = invite.response();
        let inviter_keys = self.verifying_keys(response.inviter_email()).await?;
        let invited = self.address_keys(response.invited_address_id())?.clone();

        let (pgp, account) = (*self.pgp(), Arc::clone(&self.keys));
        let invite_response: UserInviteResponse = response.clone();
        let keys = rikka_proton_api::offload(move || {
            crate::crypto::invite::reencrypt_for_account(
                &pgp,
                &invited,
                &account,
                invite_response.inviter_email(),
                &inviter_keys,
                invite_response.keys(),
            )
        })
        .await?;

        let token = invite.token();
        let envelope: ShareResponse = self
            .api()
            .post(format!("{PREFIX}/invite/{token}"))
            .json(&InviteKeysRequest { keys })
            .send_field("Share")
            .await?;

        self.decrypt_share(envelope).await
    }

    pub async fn reject_invite(&self, invite: &InviteToken) -> Result<()> {
        self.api()
            .delete(format!("{PREFIX}/invite/{invite}"))
            .send_unit()
            .await?;

        Ok(())
    }

    async fn inviter_context(&self, share: &ShareId) -> Result<InviterContext> {
        let response = self.share_response(share).await?;
        if !matches!(response.target(), ShareTarget::Vault | ShareTarget::Item) {
            return Err(Error::NotAVault {
                share: share.clone(),
                target: response.target().as_str(),
            });
        }

        let keys = self.address_keys(response.address_id())?.clone();
        let share_keys = self.refresh_share_keys(share).await?;

        Ok(InviterContext {
            share: response,
            keys,
            share_keys,
            item_keys: HashMap::new(),
        })
    }

    async fn prepare_invite(
        &self,
        inviter: &mut InviterContext,
        invitation: Invitation,
    ) -> Result<PreparedInvite> {
        let (target, item) = match (inviter.share.target(), invitation.item()) {
            (ShareTarget::Item, _) => (
                ShareTarget::Item,
                Some(ItemId::from(inviter.share.target_id())),
            ),
            (_, Some(item)) => (ShareTarget::Item, Some(item.clone())),
            (_, None) => (ShareTarget::Vault, None),
        };
        let role_id = invitation.role().id();
        let expiration_time = invitation.expires_at();
        let email = invitation.email().to_owned();

        let Some(published_keys) = self.proton_keys(&email).await? else {
            let latest = inviter
                .share_keys
                .latest()
                .ok_or_else(|| Error::NoShareKey {
                    share: inviter.share.share_id().clone(),
                })?
                .key()
                .clone();
            let (pgp, signer) = (*self.pgp(), inviter.keys.signer().clone());
            let signed_email = email.clone();
            let signature = rikka_proton_api::offload(move || {
                crate::crypto::invite::new_user_signature(&pgp, &signer, &signed_email, &latest)
            })
            .await?;

            return Ok(PreparedInvite::NewUser(CreateNewUserInviteRequest {
                email,
                target,
                signature,
                role_id,
                item_id: item,
                expiration_time,
            }));
        };

        let invitee = select_encryption_key(&email, &published_keys)?;
        let keys = match (&item, inviter.share.target()) {
            (Some(item), ShareTarget::Vault) => self.invited_item_keys(inviter, item).await?,
            _ => rotation_keys(&inviter.share_keys),
        };

        let (pgp, signer) = (*self.pgp(), inviter.keys.signer().clone());
        let sealed = rikka_proton_api::offload(move || {
            crate::crypto::invite::encrypt_for_invitee(&pgp, &signer, &invitee, &keys)
        })
        .await?;

        Ok(PreparedInvite::Existing(CreateInviteRequest {
            keys: sealed,
            email,
            target,
            role_id,
            item_id: item,
            expiration_time,
        }))
    }

    async fn invited_item_keys(
        &self,
        inviter: &mut InviterContext,
        item: &ItemId,
    ) -> Result<Vec<RotatedKey>> {
        if let Some(keys) = inviter.item_keys.get(item) {
            return Ok(keys.clone());
        }

        let keys: Vec<RotatedKey> = self
            .item_keys(inviter.share.share_id(), item)
            .await?
            .into_iter()
            .map(RotatedKey::from)
            .collect();
        inviter.item_keys.insert(item.clone(), keys.clone());

        Ok(keys)
    }
}

fn rotation_keys(keys: &ShareKeys) -> Vec<RotatedKey> {
    keys.keys().iter().cloned().map(RotatedKey::from).collect()
}

fn present<'a>(pairs: &[(&'static str, Option<&'a str>)]) -> Vec<(&'static str, &'a str)> {
    pairs
        .iter()
        .filter_map(|(name, value)| value.map(|value| (*name, value)))
        .collect()
}
