use std::collections::{HashMap, HashSet};

use futures_util::{StreamExt, TryStreamExt, stream};
use rikka_proton_api::offload;
use rikka_proton_pgp::{Pgp, PublicKey, SignatureStatus};

use crate::anchor::anchored;

use crate::client::DriveClient;
use crate::crypto;
use crate::error::{Error, Result};
use crate::identifier::{InvitationId, ShareId};
use crate::role::Role;
use crate::share_keys::ShareKeys;
use crate::sharing::invitation::{
    ExternalInvitation, Invitation, Invited, PendingInvitation, ReceivedInvitation,
};
use crate::sharing::wire::{
    AcceptRequest, EmailDetails, ExternalInvitationResponse, ExternalInvitationWire,
    ExternalInvitationsResponse, InvitationResponse, InvitationWire, InvitationsResponse,
    InviteExternalRequest, InviteRequest, NewExternalInvitation, NewInvitation, PendingResponse,
    ReceivedResponse, RoleRequest,
};

pub(super) const INVITER_CONTEXT: &str = "drive.share-member.inviter";
pub(super) const MEMBER_CONTEXT: &str = "drive.share-member.member";
const EXTERNAL_CONTEXT: &str = "drive.share-member.external-invitation";
const CONCURRENT_INVITER_LOOKUPS: usize = 8;

impl DriveClient {
    pub fn invite<'a>(&'a self, share: &'a ShareKeys, email: &'a str, role: Role) -> Invite<'a> {
        Invite {
            client: self,
            share,
            email,
            role,
            message: None,
            item_name: None,
        }
    }

    pub async fn invitations(&self, share: &ShareId) -> Result<Vec<Invitation>> {
        let response: InvitationsResponse = self
            .api()
            .get(format!("drive/v2/shares/{share}/invitations"))
            .send()
            .await?;
        let keys = self
            .inviter_keys(
                response
                    .invitations
                    .iter()
                    .map(|listed_invitation| listed_invitation.inviter.as_str()),
            )
            .await?;
        let pgp = *self.pgp();
        let share = share.clone();

        Ok(response
            .invitations
            .into_iter()
            .map(|wire| verify_invitation(&pgp, &keys, &share, wire))
            .collect())
    }

    pub async fn external_invitations(&self, share: &ShareKeys) -> Result<Vec<ExternalInvitation>> {
        let response: ExternalInvitationsResponse = self
            .api()
            .get(format!(
                "drive/v2/shares/{}/external-invitations",
                share.id()
            ))
            .send()
            .await?;
        let keys = self
            .inviter_keys(
                response
                    .invitations
                    .iter()
                    .map(|listed_invitation| listed_invitation.inviter.as_str()),
            )
            .await?;
        let pgp = *self.pgp();

        Ok(response
            .invitations
            .into_iter()
            .map(|wire| verify_external_invitation(&pgp, &keys, share, wire))
            .collect())
    }

    pub async fn update_invitation(
        &self,
        share: &ShareId,
        invitation: &InvitationId,
        role: Role,
    ) -> Result<()> {
        self.api()
            .put(format!("drive/v2/shares/{share}/invitations/{invitation}"))
            .json(&RoleRequest { role })
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn resend_invitation(
        &self,
        share: &ShareId,
        invitation: &InvitationId,
    ) -> Result<()> {
        self.api()
            .post(format!(
                "drive/v2/shares/{share}/invitations/{invitation}/sendemail"
            ))
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn delete_invitation(
        &self,
        share: &ShareId,
        invitation: &InvitationId,
    ) -> Result<()> {
        self.api()
            .delete(format!("drive/v2/shares/{share}/invitations/{invitation}"))
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn update_external_invitation(
        &self,
        share: &ShareId,
        invitation: &InvitationId,
        role: Role,
    ) -> Result<()> {
        self.api()
            .put(format!(
                "drive/v2/shares/{share}/external-invitations/{invitation}"
            ))
            .json(&RoleRequest { role })
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn resend_external_invitation(
        &self,
        share: &ShareId,
        invitation: &InvitationId,
    ) -> Result<()> {
        self.api()
            .post(format!(
                "drive/v2/shares/{share}/external-invitations/{invitation}/sendemail"
            ))
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn delete_external_invitation(
        &self,
        share: &ShareId,
        invitation: &InvitationId,
    ) -> Result<()> {
        self.api()
            .delete(format!(
                "drive/v2/shares/{share}/external-invitations/{invitation}"
            ))
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn convert_external_invitation(
        &self,
        share: &ShareKeys,
        invitation: &ExternalInvitation,
    ) -> Result<Invitation> {
        let invitee = self.encryption_key(invitation.invitee()).await?;
        self.invite_member(
            share,
            invitation.invitee(),
            invitation.role(),
            invitee,
            None,
            Some(invitation.id()),
        )
        .await
    }

    pub async fn pending_invitations(&self) -> Result<Vec<PendingInvitation>> {
        Ok(self
            .api()
            .get("drive/v2/shares/invitations")
            .pages(anchored())
            .collect(|response: PendingResponse| response.anchor.page(response.invitations))
            .await?)
    }

    pub async fn received_invitation(&self, id: &InvitationId) -> Result<ReceivedInvitation> {
        let response: ReceivedResponse = self
            .api()
            .get(format!("drive/v2/shares/invitations/{id}"))
            .send()
            .await?;
        let keys = self
            .inviter_keys(std::iter::once(response.invitation.inviter.as_str()))
            .await?;
        let client = self.clone();

        offload(move || client.decrypt_invitation(response, &keys)).await
    }

    pub async fn accept_invitation(&self, invitation: &ReceivedInvitation) -> Result<()> {
        let address = self
            .keys()
            .address_by_email(invitation.invitation().invitee())
            .ok_or_else(|| Error::NotOwnAddress(invitation.invitation().invitee().to_owned()))?
            .id()
            .clone();
        let address_keys = self.keys().address_ring(&address)?;
        let signer = crypto::signer(self.keys(), &address)?;
        let packet = rikka_proton_api::base64::decode(&invitation.key_packet)
            .map_err(|_| Error::NotBase64("invitation key packet"))?;
        let session_key =
            crypto::decrypt_session_key(self.pgp(), address_keys.private_keys(), &packet)?;
        let signature =
            crypto::context_signature(self.pgp(), &signer, session_key.as_bytes(), MEMBER_CONTEXT)?;

        self.api()
            .post(format!(
                "drive/v2/shares/invitations/{}/accept",
                invitation.invitation().id()
            ))
            .json(&AcceptRequest {
                session_key_signature: rikka_proton_api::base64::encode(signature),
            })
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn reject_invitation(&self, id: &InvitationId) -> Result<()> {
        self.api()
            .post(format!("drive/v2/shares/invitations/{id}/reject"))
            .send_unit()
            .await?;
        Ok(())
    }

    pub(super) async fn inviter_keys<'a>(
        &self,
        emails: impl Iterator<Item = &'a str>,
    ) -> Result<HashMap<String, Vec<PublicKey>>> {
        let unique: HashSet<&str> = emails.collect();
        stream::iter(unique)
            .map(|email| async move {
                let keys = self.verifying_keys(email).await?;
                Ok::<_, Error>((email.to_owned(), keys))
            })
            .buffer_unordered(CONCURRENT_INVITER_LOOKUPS)
            .try_collect()
            .await
    }

    async fn encryption_key(&self, email: &str) -> Result<PublicKey> {
        self.published_keys(email)
            .await?
            .encryption
            .ok_or_else(|| Error::NoPublicKey(email.to_owned()))
    }

    async fn invite_member(
        &self,
        share: &ShareKeys,
        email: &str,
        role: Role,
        invitee: PublicKey,
        details: Option<EmailDetails<'_>>,
        external: Option<&InvitationId>,
    ) -> Result<Invitation> {
        let inviter = crypto::email(self.keys(), share.address_id())?;
        let signer = crypto::signer(self.keys(), share.address_id())?;
        let packet = crypto::key_packet(self.pgp(), share.session_key(), &invitee)?;
        let signature = crypto::context_signature(self.pgp(), &signer, &packet, INVITER_CONTEXT)?;

        let response: InvitationResponse = self
            .api()
            .post(format!("drive/v2/shares/{}/invitations", share.id()))
            .json(&InviteRequest {
                invitation: NewInvitation {
                    inviter: &inviter,
                    invitee: email,
                    role,
                    key_packet: rikka_proton_api::base64::encode(&packet),
                    key_packet_signature: rikka_proton_api::base64::encode(signature),
                    external_invitation_id: external,
                },
                email: details,
            })
            .send()
            .await?;

        let keys = HashMap::from([(inviter, vec![signer.public_key()])]);
        Ok(verify_invitation(
            self.pgp(),
            &keys,
            share.id(),
            response.invitation,
        ))
    }

    async fn invite_external(
        &self,
        share: &ShareKeys,
        email: &str,
        role: Role,
        details: Option<EmailDetails<'_>>,
    ) -> Result<ExternalInvitation> {
        let inviter = crypto::email(self.keys(), share.address_id())?;
        let signer = crypto::signer(self.keys(), share.address_id())?;
        let payload = external_payload(email, share.session_key().as_bytes());
        let signature =
            crypto::context_signature(self.pgp(), &signer, payload.as_bytes(), EXTERNAL_CONTEXT)?;

        let response: ExternalInvitationResponse = self
            .api()
            .post(format!(
                "drive/v2/shares/{}/external-invitations",
                share.id()
            ))
            .json(&InviteExternalRequest {
                invitation: NewExternalInvitation {
                    inviter_address_id: share.address_id(),
                    invitee: email,
                    role,
                    signature: rikka_proton_api::base64::encode(signature),
                },
                email: details,
            })
            .send()
            .await?;

        let keys = HashMap::from([(inviter, vec![signer.public_key()])]);
        Ok(verify_external_invitation(
            self.pgp(),
            &keys,
            share,
            response.invitation,
        ))
    }

    fn decrypt_invitation(
        &self,
        response: ReceivedResponse,
        keys: &HashMap<String, Vec<PublicKey>>,
    ) -> Result<ReceivedInvitation> {
        let ReceivedResponse {
            invitation,
            share,
            link,
        } = response;
        let address = self
            .keys()
            .address_by_email(&invitation.invitee)
            .ok_or_else(|| Error::NotOwnAddress(invitation.invitee.clone()))?;
        let address_keys = self.keys().address_ring(address.id())?;
        let packet = rikka_proton_api::base64::decode(&invitation.key_packet)
            .map_err(|_| Error::NotBase64("invitation key packet"))?;
        let session_key =
            crypto::decrypt_session_key(self.pgp(), address_keys.private_keys(), &packet)?;
        let share_key = crypto::decrypt_key_with_session_key(
            self.pgp(),
            &share.key,
            &share.passphrase,
            session_key,
            None,
            &[],
        )?;
        let name = crypto::decrypt_name(self.pgp(), &share_key.key, &[], &link.name)?.name;
        let key_packet = invitation.key_packet.clone();

        Ok(ReceivedInvitation {
            invitation: verify_invitation(self.pgp(), keys, &share.id, invitation),
            volume_id: share.volume_id,
            link_id: link.id,
            link_type: link.kind,
            media_type: link
                .media_type
                .filter(|listed_invitation| !listed_invitation.is_empty()),
            target: share.target,
            creator: share.creator,
            name,
            key_packet,
        })
    }
}

#[must_use]
#[derive(Debug)]
pub struct Invite<'a> {
    client: &'a DriveClient,
    share: &'a ShareKeys,
    email: &'a str,
    role: Role,
    message: Option<&'a str>,
    item_name: Option<&'a str>,
}

impl<'a> Invite<'a> {
    pub fn message(mut self, message: &'a str) -> Self {
        self.message = Some(message);
        self
    }

    pub fn item_name(mut self, name: &'a str) -> Self {
        self.item_name = Some(name);
        self
    }

    pub async fn send(self) -> Result<Invited> {
        let details =
            (self.message.is_some() || self.item_name.is_some()).then_some(EmailDetails {
                message: self.message,
                item_name: self.item_name,
            });
        let published = self.client.published_keys(self.email).await?;

        match published.encryption {
            Some(invitee) => self
                .client
                .invite_member(self.share, self.email, self.role, invitee, details, None)
                .await
                .map(Invited::Member),
            None => self
                .client
                .invite_external(self.share, self.email, self.role, details)
                .await
                .map(Invited::External),
        }
    }
}

fn external_payload(email: &str, session_key: &[u8]) -> String {
    format!("{email}|{}", rikka_proton_api::base64::encode(session_key))
}

pub(super) fn verify_key_packet(
    pgp: &Pgp,
    keys: &[PublicKey],
    packet: &str,
    signature: &str,
    context: &str,
) -> SignatureStatus {
    match (
        rikka_proton_api::base64::decode(packet),
        rikka_proton_api::base64::decode(signature),
    ) {
        (Ok(packet), Ok(signature)) => {
            crypto::verify_context(pgp, keys, &packet, &signature, context)
        }
        _ => SignatureStatus::Invalid {
            key_id: None,
            reason: "key packet is not valid base64".to_owned(),
        },
    }
}

fn verify_invitation(
    pgp: &Pgp,
    keys: &HashMap<String, Vec<PublicKey>>,
    share: &ShareId,
    wire: InvitationWire,
) -> Invitation {
    let verification = keys.get(&wire.inviter).map_or(&[][..], Vec::as_slice);
    let signature = verify_key_packet(
        pgp,
        verification,
        &wire.key_packet,
        &wire.key_packet_signature,
        INVITER_CONTEXT,
    );

    Invitation {
        id: wire.id,
        share_id: share.clone(),
        inviter: wire.inviter,
        invitee: wire.invitee,
        role: wire.role,
        created_at: wire.created_at,
        signature,
    }
}

fn verify_external_invitation(
    pgp: &Pgp,
    keys: &HashMap<String, Vec<PublicKey>>,
    share: &ShareKeys,
    wire: ExternalInvitationWire,
) -> ExternalInvitation {
    let verification = keys.get(&wire.inviter).map_or(&[][..], Vec::as_slice);
    let payload = external_payload(&wire.invitee, share.session_key().as_bytes());
    let signature = match rikka_proton_api::base64::decode(&wire.signature) {
        Ok(signature) => crypto::verify_context(
            pgp,
            verification,
            payload.as_bytes(),
            &signature,
            EXTERNAL_CONTEXT,
        ),
        Err(_) => SignatureStatus::Invalid {
            key_id: None,
            reason: "external invitation signature is not valid base64".to_owned(),
        },
    };

    ExternalInvitation {
        id: wire.id,
        share_id: share.id().clone(),
        inviter: wire.inviter,
        invitee: wire.invitee,
        role: wire.role,
        state: wire.state,
        created_at: wire.created_at,
        signature,
    }
}
