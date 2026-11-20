use jiff::Timestamp;
use rikka_proton_api::wire_enum;
use rikka_proton_pgp::SignatureStatus;
use serde::Deserialize;

use crate::identifier::{InvitationId, LinkId, MemberId, ShareId, VolumeId};
use crate::link::LinkType;
use crate::role::{Role, ShareTarget};

wire_enum! {
    pub enum ExternalInvitationState: default Pending {
        Pending = 1,
        UserRegistered = 2,
    }
}

#[derive(Debug, Clone)]
pub struct Invitation {
    pub(super) id: InvitationId,
    pub(super) share_id: ShareId,
    pub(super) inviter: String,
    pub(super) invitee: String,
    pub(super) role: Role,
    pub(super) created_at: Option<Timestamp>,
    pub(super) signature: SignatureStatus,
}

impl Invitation {
    #[must_use]
    pub fn id(&self) -> &InvitationId {
        &self.id
    }

    #[must_use]
    pub fn share_id(&self) -> &ShareId {
        &self.share_id
    }

    #[must_use]
    pub fn inviter(&self) -> &str {
        &self.inviter
    }

    #[must_use]
    pub fn invitee(&self) -> &str {
        &self.invitee
    }

    #[must_use]
    pub fn role(&self) -> Role {
        self.role
    }

    #[must_use]
    pub fn created_at(&self) -> Option<Timestamp> {
        self.created_at
    }

    #[must_use]
    pub fn signature(&self) -> &SignatureStatus {
        &self.signature
    }
}

#[derive(Debug, Clone)]
pub struct ExternalInvitation {
    pub(super) id: InvitationId,
    pub(super) share_id: ShareId,
    pub(super) inviter: String,
    pub(super) invitee: String,
    pub(super) role: Role,
    pub(super) state: ExternalInvitationState,
    pub(super) created_at: Option<Timestamp>,
    pub(super) signature: SignatureStatus,
}

impl ExternalInvitation {
    #[must_use]
    pub fn id(&self) -> &InvitationId {
        &self.id
    }

    #[must_use]
    pub fn share_id(&self) -> &ShareId {
        &self.share_id
    }

    #[must_use]
    pub fn inviter(&self) -> &str {
        &self.inviter
    }

    #[must_use]
    pub fn invitee(&self) -> &str {
        &self.invitee
    }

    #[must_use]
    pub fn role(&self) -> Role {
        self.role
    }

    #[must_use]
    pub fn state(&self) -> ExternalInvitationState {
        self.state
    }

    #[must_use]
    pub fn created_at(&self) -> Option<Timestamp> {
        self.created_at
    }

    #[must_use]
    pub fn signature(&self) -> &SignatureStatus {
        &self.signature
    }
}

#[derive(Debug, Clone)]
pub enum Invited {
    Member(Invitation),
    External(ExternalInvitation),
}

#[derive(Debug, Clone)]
pub struct Member {
    pub(super) id: MemberId,
    pub(super) share_id: ShareId,
    pub(super) inviter: String,
    pub(super) email: String,
    pub(super) role: Role,
    pub(super) created_at: Option<Timestamp>,
    pub(super) signature: SignatureStatus,
}

impl Member {
    #[must_use]
    pub fn id(&self) -> &MemberId {
        &self.id
    }

    #[must_use]
    pub fn share_id(&self) -> &ShareId {
        &self.share_id
    }

    #[must_use]
    pub fn inviter(&self) -> &str {
        &self.inviter
    }

    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    #[must_use]
    pub fn role(&self) -> Role {
        self.role
    }

    #[must_use]
    pub fn created_at(&self) -> Option<Timestamp> {
        self.created_at
    }

    #[must_use]
    pub fn signature(&self) -> &SignatureStatus {
        &self.signature
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PendingInvitation {
    #[serde(rename = "InvitationID")]
    id: InvitationId,
    #[serde(rename = "VolumeID")]
    volume_id: VolumeId,
    #[serde(rename = "ShareID")]
    share_id: ShareId,
    #[serde(rename = "ShareTargetType", default)]
    target: ShareTarget,
}

impl PendingInvitation {
    #[must_use]
    pub fn id(&self) -> &InvitationId {
        &self.id
    }

    #[must_use]
    pub fn volume_id(&self) -> &VolumeId {
        &self.volume_id
    }

    #[must_use]
    pub fn share_id(&self) -> &ShareId {
        &self.share_id
    }

    #[must_use]
    pub fn target(&self) -> ShareTarget {
        self.target
    }
}

#[derive(Debug, Clone)]
pub struct ReceivedInvitation {
    pub(super) invitation: Invitation,
    pub(super) volume_id: VolumeId,
    pub(super) link_id: LinkId,
    pub(super) link_type: LinkType,
    pub(super) media_type: Option<String>,
    pub(super) target: ShareTarget,
    pub(super) creator: String,
    pub(super) name: String,
    pub(super) key_packet: String,
}

impl ReceivedInvitation {
    #[must_use]
    pub fn invitation(&self) -> &Invitation {
        &self.invitation
    }

    #[must_use]
    pub fn volume_id(&self) -> &VolumeId {
        &self.volume_id
    }

    #[must_use]
    pub fn link_id(&self) -> &LinkId {
        &self.link_id
    }

    #[must_use]
    pub fn link_type(&self) -> LinkType {
        self.link_type
    }

    #[must_use]
    pub fn media_type(&self) -> Option<&str> {
        self.media_type.as_deref()
    }

    #[must_use]
    pub fn target(&self) -> ShareTarget {
        self.target
    }

    #[must_use]
    pub fn creator(&self) -> &str {
        &self.creator
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}
