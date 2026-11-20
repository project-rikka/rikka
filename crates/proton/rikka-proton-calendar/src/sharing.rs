use futures_util::{StreamExt, TryStreamExt, stream};
use rikka_proton_account::AddressId;
use rikka_proton_api::{base64, offload};
use rikka_proton_contacts::MailDefaults;
use rikka_proton_pgp::{PublicKey, SignatureContext, VerificationContext};
use serde::Serialize;

use crate::calendar::Permissions;
use crate::client::{CALENDAR_V1, CalendarClient, MAX_CONCURRENT_LOOKUPS};
use crate::error::{Error, Result};
use crate::identifier::{CalendarId, InvitationId, MemberId};
use crate::invitation::CalendarInvitation;
use crate::opening::armored_session_key;
use crate::unlocked::{UnlockedCalendar, passphrase_signature};

const INVITE_CONTEXT: &str = "calendar.sharing.invite";

#[derive(Serialize)]
struct InvitedMember {
    #[serde(rename = "Email")]
    email: String,
    #[serde(rename = "PassphraseKeyPacket")]
    key_packet: String,
    #[serde(rename = "Permissions")]
    permissions: Permissions,
}

#[derive(Serialize)]
struct ShareBody<'a> {
    #[serde(rename = "AddressID")]
    address_id: &'a AddressId,
    #[serde(rename = "Signature")]
    signature: String,
    #[serde(rename = "Members")]
    members: Vec<InvitedMember>,
}

#[derive(Serialize)]
struct PermissionsBody {
    #[serde(rename = "Permissions")]
    permissions: Permissions,
}

#[derive(Serialize)]
struct SignatureBody {
    #[serde(rename = "Signature")]
    signature: String,
}

#[must_use]
#[derive(Debug, Clone)]
pub struct ShareBuilder {
    client: CalendarClient,
    calendar: UnlockedCalendar,
    invitees: Vec<(String, Permissions)>,
}

impl ShareBuilder {
    pub fn invite(mut self, email: impl Into<String>, permissions: Permissions) -> ShareBuilder {
        self.invitees.push((email.into(), permissions));
        self
    }

    pub async fn send(self) -> Result<Vec<CalendarInvitation>> {
        let client = &self.client;
        let recipients: Vec<(String, Permissions, PublicKey)> = stream::iter(self.invitees)
            .map(|(email, permissions)| async move {
                let key = client.encryption_key(&email).await?;
                Ok::<_, Error>((email, permissions, key))
            })
            .buffered(MAX_CONCURRENT_LOOKUPS)
            .try_collect()
            .await?;

        let pgp = *self.client.pgp();
        let session = self.calendar.passphrase_session()?.clone();
        let signer = self.calendar.address_key().clone();
        let (signature, members) = offload(move || -> Result<(String, Vec<InvitedMember>)> {
            let signature = pgp
                .sign()
                .with_key(signer)
                .with_context(SignatureContext::new(INVITE_CONTEXT).critical(true))
                .armored_detached(session.as_bytes())?;
            let members = recipients
                .into_iter()
                .map(|(email, permissions, key)| {
                    Ok(InvitedMember {
                        email,
                        key_packet: base64::encode(pgp.encrypt().to(key).key_packets(&session)?),
                        permissions,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            Ok((signature, members))
        })
        .await?;

        Ok(self
            .client
            .api()
            .post(format!(
                "{CALENDAR_V1}/{}/members",
                self.calendar.calendar_id()
            ))
            .json(&ShareBody {
                address_id: self.calendar.address_id(),
                signature,
                members,
            })
            .send_field_or_default("Invitations")
            .await?)
    }
}

impl CalendarClient {
    pub fn share(&self, calendar: &UnlockedCalendar) -> ShareBuilder {
        ShareBuilder {
            client: self.clone(),
            calendar: calendar.clone(),
            invitees: Vec::new(),
        }
    }

    pub async fn sent_invitations(&self, calendar: &CalendarId) -> Result<Vec<CalendarInvitation>> {
        Ok(self
            .api()
            .get(format!("{CALENDAR_V1}/{calendar}/invitations"))
            .send_field_or_default("Invitations")
            .await?)
    }

    pub async fn received_invitations(&self) -> Result<Vec<CalendarInvitation>> {
        Ok(self
            .api()
            .get(format!("{CALENDAR_V1}/invitations"))
            .send_field_or_default("Invitations")
            .await?)
    }

    pub async fn update_member_permissions(
        &self,
        calendar: &CalendarId,
        member: &MemberId,
        permissions: Permissions,
    ) -> Result<()> {
        self.api()
            .put(format!(
                "{CALENDAR_V1}/{calendar}/members/{member}/permission"
            ))
            .json(&PermissionsBody { permissions })
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn update_invitation_permissions(
        &self,
        calendar: &CalendarId,
        invitation: &InvitationId,
        permissions: Permissions,
    ) -> Result<()> {
        self.api()
            .put(format!("{CALENDAR_V1}/{calendar}/invitations/{invitation}"))
            .json(&PermissionsBody { permissions })
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn remove_member(&self, calendar: &CalendarId, member: &MemberId) -> Result<()> {
        self.api()
            .delete(format!("{CALENDAR_V1}/{calendar}/members/{member}"))
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn remove_invitation(
        &self,
        calendar: &CalendarId,
        invitation: &InvitationId,
    ) -> Result<()> {
        self.api()
            .delete(format!("{CALENDAR_V1}/{calendar}/invitations/{invitation}"))
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn invitation_sender_keys(
        &self,
        invitation: &CalendarInvitation,
    ) -> Result<Vec<PublicKey>> {
        let sender = invitation.calendar().sender_email();
        if sender.trim().is_empty() {
            return Ok(Vec::new());
        }
        Ok(self
            .contacts()
            .encryption_preferences(sender, &MailDefaults::default())
            .await?
            .verifying_pinned_keys)
    }

    pub async fn accept_invitation(
        &self,
        invitation: &CalendarInvitation,
        sender_keys: &[PublicKey],
    ) -> Result<()> {
        let address = self
            .keys()
            .address_by_email(invitation.email())
            .ok_or_else(|| Error::NotInvited(invitation.email().to_owned()))?;
        let address_id = address.id().clone();
        let ring = self.keys().address_ring(&address_id)?.clone();
        let key = self.keys().signer(&address_id)?.clone();
        let pgp = *self.pgp();
        let armored = invitation.armored_passphrase().to_owned();
        let invitation_signature = invitation.signature().to_owned();
        let verifiers = sender_keys.to_vec();

        let signature = offload(move || -> Result<String> {
            let session = armored_session_key(&pgp, &armored, &ring)?;
            let decrypted = pgp
                .decrypt()
                .with_session_key(session.clone())
                .as_utf8()
                .message(&armored)?;
            if !verifiers.is_empty() {
                let context_status = pgp
                    .verify()
                    .with_keys(verifiers.clone())
                    .with_context(VerificationContext::new(INVITE_CONTEXT).required(true))
                    .detached(session.as_bytes(), &invitation_signature);
                let legacy_status = || {
                    pgp.verify()
                        .with_keys(verifiers.clone())
                        .as_utf8()
                        .detached(decrypted.data(), &invitation_signature)
                };
                if !context_status.is_valid() && !legacy_status().is_valid() {
                    return Err(Error::PassphraseSignature);
                }
            }
            passphrase_signature(&pgp, decrypted.data(), &key)
        })
        .await?;

        self.api()
            .put(format!(
                "{CALENDAR_V1}/{}/invitations/{address_id}/accept",
                invitation.calendar_id()
            ))
            .json(&SignatureBody { signature })
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn reject_invitation(&self, invitation: &CalendarInvitation) -> Result<()> {
        let address = self
            .keys()
            .address_by_email(invitation.email())
            .ok_or_else(|| Error::NotInvited(invitation.email().to_owned()))?;
        self.api()
            .put(format!(
                "{CALENDAR_V1}/{}/invitations/{}/reject",
                invitation.calendar_id(),
                address.id()
            ))
            .send_unit()
            .await?;
        Ok(())
    }
}
