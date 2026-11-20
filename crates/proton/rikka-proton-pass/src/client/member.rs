use rikka_proton_api::{Page, Paging};

use crate::api::{
    MemberRole, MembersEnvelope, ShareMember, TransferOwnershipRequest, UpdateMemberRequest,
};
use crate::client::{PAGE_SIZE, PREFIX, PassClient};
use crate::error::Result;
use crate::identifier::{ItemId, ShareId};

impl PassClient {
    pub async fn members(&self, share: &ShareId) -> Result<Vec<ShareMember>> {
        self.paged_members(format!("{PREFIX}/share/{share}/user"))
            .await
    }

    pub async fn item_members(&self, share: &ShareId, item: &ItemId) -> Result<Vec<ShareMember>> {
        self.paged_members(format!("{PREFIX}/share/{share}/user/item/{item}"))
            .await
    }

    pub async fn set_member_role(
        &self,
        share: &ShareId,
        member: &ShareId,
        role: MemberRole,
        expires_at: Option<jiff::Timestamp>,
    ) -> Result<()> {
        let request = UpdateMemberRequest {
            role_id: role.id(),
            expire_time: expires_at,
        };

        self.api()
            .put(format!("{PREFIX}/share/{share}/user/{member}"))
            .json(&request)
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn remove_member(&self, share: &ShareId, member: &ShareId) -> Result<()> {
        self.api()
            .delete(format!("{PREFIX}/share/{share}/user/{member}"))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn transfer_ownership(&self, share: &ShareId, new_owner: &ShareId) -> Result<()> {
        self.api()
            .put(format!("{PREFIX}/vault/{share}/owner"))
            .json(&TransferOwnershipRequest { new_owner })
            .send_unit()
            .await?;

        Ok(())
    }

    async fn paged_members(&self, path: String) -> Result<Vec<ShareMember>> {
        Ok(self
            .api()
            .get(path)
            .pages(Paging::since().page_size(PAGE_SIZE))
            .collect(|envelope: MembersEnvelope| {
                Page::new(envelope.shares).next(envelope.last_token)
            })
            .await?)
    }
}
