use std::collections::HashMap;

use rikka_proton_account::AddressKey;
use rikka_proton_api::{Page, Paging};
use rikka_proton_pgp::{PrivateKey, PublicKey};

use crate::api::{
    Group, GroupInvitePage, GroupInviteResponse, GroupMember, GroupMembersEnvelope,
    InviteKeysRequest,
};
use crate::client::{PREFIX, PassClient};
use crate::crypto::group::GroupKeyUnlocker;
use crate::error::{Error, Result};
use crate::identifier::{GroupId, InviteToken};
use crate::pending_group_invite::PendingGroupInvite;

const MEMBER_PAGE: u32 = 1000;

impl PassClient {
    pub async fn groups(&self) -> Result<Vec<Group>> {
        let envelope: Vec<Group> = self
            .api()
            .get("core/v4/groups")
            .send_field_or_default("Groups")
            .await?;

        Ok(envelope)
    }

    pub async fn group(&self, group: &GroupId) -> Result<Group> {
        let envelope: Group = self
            .api()
            .get(format!("core/v4/groups/{group}"))
            .send_field("Group")
            .await?;

        Ok(envelope)
    }

    pub async fn group_members(&self, group: &GroupId) -> Result<Vec<GroupMember>> {
        Ok(self
            .api()
            .get(format!("core/v4/groups/{group}/members"))
            .pages(Paging::numbered(MEMBER_PAGE))
            .collect(|envelope: GroupMembersEnvelope| {
                Page::new(envelope.members).total(envelope.total)
            })
            .await?)
    }

    pub async fn group_invites(&self) -> Result<Vec<PendingGroupInvite>> {
        let invites = self
            .api()
            .get(format!("{PREFIX}/invite/group"))
            .pages(Paging::since())
            .field("Invites")
            .collect(|page: GroupInvitePage| Page::new(page.invites).next(page.last_id))
            .await?;
        self.decrypt_group_invites(invites).await
    }

    async fn decrypt_group_invites(
        &self,
        invites: Vec<GroupInviteResponse>,
    ) -> Result<Vec<PendingGroupInvite>> {
        if invites.is_empty() {
            return Ok(Vec::new());
        }

        let groups = self.groups().await?;
        let organization = if invites.iter().any(|invite| !invite.is_group_owner()) {
            match self.decrypt_organization_key().await {
                Err(Error::Api(error)) => return Err(Error::Api(error)),
                decrypted => Some(decrypted),
            }
        } else {
            None
        };

        let mut inviters: HashMap<String, Vec<PublicKey>> = HashMap::new();
        let mut decrypted = Vec::with_capacity(invites.len());

        for invite in invites {
            let Some(data) = invite.vault_data() else {
                decrypted.push(PendingGroupInvite::new(invite, None));
                continue;
            };
            let Some(key) = invite
                .keys()
                .iter()
                .find(|key| key.rotation() == data.content_key_rotation())
                .cloned()
            else {
                tracing::warn!(token = %invite.token(), "group invite has no content key");
                continue;
            };

            let Some(unlocker) = self.group_key_unlocker(&invite, organization.as_ref()) else {
                continue;
            };
            let Some(keys) = group_keys(&groups, invite.group_id()) else {
                tracing::warn!(token = %invite.token(), "group invite names unknown group");
                continue;
            };
            let Some(inviter_keys) = self.inviter_keys(&mut inviters, &invite).await? else {
                continue;
            };

            let pgp = *self.pgp();
            let (vault, invite) = rikka_proton_api::offload(move || {
                let vault = invite.vault_data().map(|data| {
                    let group = crate::crypto::group::unlock_group_keys(
                        &pgp,
                        invite.group_id(),
                        &keys,
                        &unlocker,
                    )?;
                    let decrypted = crate::crypto::invite::decrypt_invite_key(
                        &pgp,
                        &group,
                        invite.inviter_email(),
                        &inviter_keys,
                        &key,
                    )?;
                    crate::crypto::invite::decrypt_vault(&decrypted, data.content())
                });
                (vault, invite)
            })
            .await;

            match vault {
                Some(Ok(vault)) => decrypted.push(PendingGroupInvite::new(invite, Some(vault))),
                Some(Err(error)) => {
                    tracing::warn!(token = %invite.token(), %error, "failed to open group invite");
                }
                None => decrypted.push(PendingGroupInvite::new(invite, None)),
            }
        }

        Ok(decrypted)
    }

    fn group_key_unlocker(
        &self,
        invite: &GroupInviteResponse,
        organization: Option<&Result<PrivateKey>>,
    ) -> Option<GroupKeyUnlocker> {
        match (invite.is_group_owner(), organization) {
            (true, _) => Some(GroupKeyUnlocker::Owner(self.keys().clone())),
            (false, Some(Ok(key))) => Some(GroupKeyUnlocker::Administrator(key.clone())),
            (false, Some(Err(error))) => {
                tracing::warn!(
                    token = %invite.token(),
                    %error,
                    "group invite requires unavailable organization key"
                );
                None
            }
            (false, None) => None,
        }
    }

    async fn inviter_keys(
        &self,
        inviters: &mut HashMap<String, Vec<PublicKey>>,
        invite: &GroupInviteResponse,
    ) -> Result<Option<Vec<PublicKey>>> {
        if let Some(keys) = inviters.get(invite.inviter_email()) {
            return Ok(Some(keys.clone()));
        }
        match self.verifying_keys(invite.inviter_email()).await {
            Ok(keys) => {
                inviters.insert(invite.inviter_email().to_owned(), keys.clone());
                Ok(Some(keys))
            }
            Err(error @ Error::NoPublicKey { .. }) => {
                tracing::warn!(token = %invite.token(), %error, "group invite inviter has no keys");
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    pub async fn accept_group_invite(&self, invite: &PendingGroupInvite) -> Result<()> {
        let response = invite.response();
        let groups = self.groups().await?;
        let keys = group_keys(&groups, response.group_id()).ok_or_else(|| Error::NoGroup {
            group: response.group_id().clone(),
        })?;
        let unlocker = if response.is_group_owner() {
            GroupKeyUnlocker::Owner(self.keys().clone())
        } else {
            GroupKeyUnlocker::Administrator(self.decrypt_organization_key().await?)
        };
        let inviter_keys = self.verifying_keys(response.inviter_email()).await?;

        let pgp = *self.pgp();
        let invite_response = response.clone();
        let sealed = rikka_proton_api::offload(move || {
            let group = crate::crypto::group::unlock_group_keys(
                &pgp,
                invite_response.group_id(),
                &keys,
                &unlocker,
            )?;
            crate::crypto::invite::reencrypt_for_account(
                &pgp,
                &group,
                &group,
                invite_response.inviter_email(),
                &inviter_keys,
                invite_response.keys(),
            )
        })
        .await?;

        let token = invite.token();
        self.api()
            .post(format!("{PREFIX}/invite/group/{token}"))
            .json(&InviteKeysRequest { keys: sealed })
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn reject_group_invite(&self, invite: &InviteToken) -> Result<()> {
        self.api()
            .delete(format!("{PREFIX}/invite/group/{invite}"))
            .send_unit()
            .await?;

        Ok(())
    }

    pub(crate) async fn group_verifying_keys(&self, group: &GroupId) -> Result<Vec<PublicKey>> {
        let group_info = self.group(group).await?;
        let email = group_info.email().ok_or_else(|| Error::NoGroup {
            group: group.clone(),
        })?;

        self.verifying_keys(email).await
    }

    async fn decrypt_organization_key(&self) -> Result<PrivateKey> {
        let keys = self.account_keys().ok_or(Error::OrganizationKeyLocked)?;

        match self.account().organization_key(keys).await {
            Ok(key) => Ok(key),
            Err(rikka_proton_account::Error::Api(error)) => Err(Error::Api(error)),
            Err(error) => Err(error.into()),
        }
    }
}

fn group_keys(groups: &[Group], group: &GroupId) -> Option<Vec<AddressKey>> {
    groups
        .iter()
        .find(|candidate| candidate.id() == group)
        .and_then(Group::address)
        .map(|address| address.keys().to_vec())
}
