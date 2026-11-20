use crate::client::DriveClient;
use crate::error::Result;
use crate::identifier::{MemberId, ShareId};
use crate::role::Role;
use crate::sharing::invitation::Member;
use crate::sharing::invitations::{INVITER_CONTEXT, verify_key_packet};
use crate::sharing::wire::{MembersResponse, RoleRequest};

impl DriveClient {
    pub async fn members(&self, share: &ShareId) -> Result<Vec<Member>> {
        let response: MembersResponse = self
            .api()
            .get(format!("drive/v2/shares/{share}/members"))
            .send()
            .await?;
        let keys = self
            .inviter_keys(
                response
                    .members
                    .iter()
                    .map(|listed_member| listed_member.inviter.as_str()),
            )
            .await?;

        Ok(response
            .members
            .into_iter()
            .map(|wire| {
                let verifying = keys.get(&wire.inviter).map_or(&[][..], Vec::as_slice);
                let signature = verify_key_packet(
                    self.pgp(),
                    verifying,
                    &wire.key_packet,
                    &wire.key_packet_signature,
                    INVITER_CONTEXT,
                );
                Member {
                    id: wire.id,
                    share_id: share.clone(),
                    inviter: wire.inviter,
                    email: wire.email,
                    role: wire.role,
                    created_at: wire.created_at,
                    signature,
                }
            })
            .collect())
    }

    pub async fn update_member(
        &self,
        share: &ShareId,
        member: &MemberId,
        role: Role,
    ) -> Result<()> {
        self.api()
            .put(format!("drive/v2/shares/{share}/members/{member}"))
            .json(&RoleRequest { role })
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn remove_member(&self, share: &ShareId, member: &MemberId) -> Result<()> {
        self.api()
            .delete(format!("drive/v2/shares/{share}/members/{member}"))
            .send_unit()
            .await?;
        Ok(())
    }
}
