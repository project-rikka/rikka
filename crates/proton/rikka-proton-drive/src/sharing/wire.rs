use jiff::Timestamp;
use rikka_proton_account::AddressId;
use serde::{Deserialize, Serialize};

use crate::anchor::Anchor;
use crate::identifier::{InvitationId, LinkId, MemberId, ShareId, VolumeId};
use crate::link::LinkType;
use crate::role::{Role, ShareTarget};
use crate::sharing::invitation::{ExternalInvitationState, PendingInvitation};

#[derive(Debug, Clone, Deserialize)]
pub(super) struct InvitationWire {
    #[serde(rename = "InvitationID")]
    pub(super) id: InvitationId,
    #[serde(rename = "InviterEmail")]
    pub(super) inviter: String,
    #[serde(rename = "InviteeEmail")]
    pub(super) invitee: String,
    #[serde(rename = "Permissions", default)]
    pub(super) role: Role,
    #[serde(rename = "KeyPacket")]
    pub(super) key_packet: String,
    #[serde(rename = "KeyPacketSignature", default)]
    pub(super) key_packet_signature: String,
    #[serde(
        rename = "CreateTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    pub(super) created_at: Option<Timestamp>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ExternalInvitationWire {
    #[serde(rename = "ExternalInvitationID")]
    pub(super) id: InvitationId,
    #[serde(rename = "InviterEmail")]
    pub(super) inviter: String,
    #[serde(rename = "InviteeEmail")]
    pub(super) invitee: String,
    #[serde(rename = "Permissions", default)]
    pub(super) role: Role,
    #[serde(rename = "ExternalInvitationSignature", default)]
    pub(super) signature: String,
    #[serde(rename = "State", default)]
    pub(super) state: ExternalInvitationState,
    #[serde(
        rename = "CreateTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    pub(super) created_at: Option<Timestamp>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct MemberWire {
    #[serde(rename = "MemberID")]
    pub(super) id: MemberId,
    #[serde(rename = "InviterEmail")]
    pub(super) inviter: String,
    #[serde(rename = "Email")]
    pub(super) email: String,
    #[serde(rename = "Permissions", default)]
    pub(super) role: Role,
    #[serde(rename = "KeyPacket")]
    pub(super) key_packet: String,
    #[serde(rename = "KeyPacketSignature", default)]
    pub(super) key_packet_signature: String,
    #[serde(
        rename = "CreateTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    pub(super) created_at: Option<Timestamp>,
}

#[derive(Debug, Deserialize)]
pub(super) struct InvitationsResponse {
    #[serde(rename = "Invitations", default)]
    pub(super) invitations: Vec<InvitationWire>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ExternalInvitationsResponse {
    #[serde(rename = "ExternalInvitations", default)]
    pub(super) invitations: Vec<ExternalInvitationWire>,
}

#[derive(Debug, Deserialize)]
pub(super) struct MembersResponse {
    #[serde(rename = "Members", default)]
    pub(super) members: Vec<MemberWire>,
}

#[derive(Debug, Deserialize)]
pub(super) struct InvitationResponse {
    #[serde(rename = "Invitation")]
    pub(super) invitation: InvitationWire,
}

#[derive(Debug, Deserialize)]
pub(super) struct ExternalInvitationResponse {
    #[serde(rename = "ExternalInvitation")]
    pub(super) invitation: ExternalInvitationWire,
}

#[derive(Debug, Deserialize)]
pub(super) struct PendingResponse {
    #[serde(rename = "Invitations", default)]
    pub(super) invitations: Vec<PendingInvitation>,
    #[serde(flatten)]
    pub(super) anchor: Anchor,
}

#[derive(Debug, Deserialize)]
pub(super) struct ReceivedResponse {
    #[serde(rename = "Invitation")]
    pub(super) invitation: InvitationWire,
    #[serde(rename = "Share")]
    pub(super) share: ReceivedShare,
    #[serde(rename = "Link")]
    pub(super) link: ReceivedLink,
}

#[derive(Debug, Deserialize)]
pub(super) struct ReceivedShare {
    #[serde(rename = "ShareID")]
    pub(super) id: ShareId,
    #[serde(rename = "VolumeID")]
    pub(super) volume_id: VolumeId,
    #[serde(rename = "Passphrase")]
    pub(super) passphrase: String,
    #[serde(rename = "ShareKey")]
    pub(super) key: String,
    #[serde(rename = "CreatorEmail", default)]
    pub(super) creator: String,
    #[serde(rename = "ShareTargetType", default)]
    pub(super) target: ShareTarget,
}

#[derive(Debug, Deserialize)]
pub(super) struct ReceivedLink {
    #[serde(rename = "Type", default)]
    pub(super) kind: LinkType,
    #[serde(rename = "LinkID")]
    pub(super) id: LinkId,
    #[serde(rename = "Name")]
    pub(super) name: String,
    #[serde(rename = "MIMEType", default)]
    pub(super) media_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct InviteRequest<'a> {
    #[serde(rename = "Invitation")]
    pub(super) invitation: NewInvitation<'a>,
    #[serde(rename = "EmailDetails")]
    pub(super) email: Option<EmailDetails<'a>>,
}

#[derive(Debug, Serialize)]
pub(super) struct NewInvitation<'a> {
    #[serde(rename = "InviterEmail")]
    pub(super) inviter: &'a str,
    #[serde(rename = "InviteeEmail")]
    pub(super) invitee: &'a str,
    #[serde(rename = "Permissions")]
    pub(super) role: Role,
    #[serde(rename = "KeyPacket")]
    pub(super) key_packet: String,
    #[serde(rename = "KeyPacketSignature")]
    pub(super) key_packet_signature: String,
    #[serde(rename = "ExternalInvitationID")]
    pub(super) external_invitation_id: Option<&'a InvitationId>,
}

#[derive(Debug, Serialize)]
pub(super) struct InviteExternalRequest<'a> {
    #[serde(rename = "ExternalInvitation")]
    pub(super) invitation: NewExternalInvitation<'a>,
    #[serde(rename = "EmailDetails")]
    pub(super) email: Option<EmailDetails<'a>>,
}

#[derive(Debug, Serialize)]
pub(super) struct NewExternalInvitation<'a> {
    #[serde(rename = "InviterAddressID")]
    pub(super) inviter_address_id: &'a AddressId,
    #[serde(rename = "InviteeEmail")]
    pub(super) invitee: &'a str,
    #[serde(rename = "Permissions")]
    pub(super) role: Role,
    #[serde(rename = "ExternalInvitationSignature")]
    pub(super) signature: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub(super) struct EmailDetails<'a> {
    #[serde(rename = "Message", skip_serializing_if = "Option::is_none")]
    pub(super) message: Option<&'a str>,
    #[serde(rename = "ItemName", skip_serializing_if = "Option::is_none")]
    pub(super) item_name: Option<&'a str>,
}

#[derive(Debug, Serialize)]
pub(super) struct RoleRequest {
    #[serde(rename = "Permissions")]
    pub(super) role: Role,
}

#[derive(Debug, Serialize)]
pub(super) struct AcceptRequest {
    #[serde(rename = "SessionKeySignature")]
    pub(super) session_key_signature: String,
}
