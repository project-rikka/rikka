use rikka_proton_api::{parallel_map_each, wire_enum};
use rikka_proton_pgp::{
    Pgp, PrivateKey, PublicKey, Secret, SignatureContext, SignatureStatus, VerificationContext,
};
use serde::{Deserialize, Serialize};

use crate::account::Account;
use crate::account_keys::AccountKeys;
use crate::error::{Error, Result};

const UNPRIVATIZE_PATH: &str = "core/v4/members/me/unprivatize";
const FINGERPRINT_CONTEXT: &str = "account.organization-fingerprint";
const INVITATION_CONTEXT: &str = "account.unprivatization-invitation-data";
const TOKEN_CONTEXT: &str = "account.key-token.user-unprivatization";
const TOKEN_LEN: usize = 32;

wire_enum! {
    pub enum UnprivatizationState: default Declined {
        Declined = 0,
        Pending = 1,
        Ready = 2,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unprivatization {
    #[serde(rename = "State", default)]
    state: UnprivatizationState,
    #[serde(rename = "InvitationData", default)]
    invitation_data: Option<String>,
    #[serde(rename = "InvitationSignature", default)]
    invitation_signature: Option<String>,
    #[serde(rename = "InvitationEmail", default)]
    invitation_email: Option<String>,
    #[serde(rename = "AdminEmail", default)]
    admin_email: String,
    #[serde(rename = "OrgKeyFingerprintSignature", default)]
    fingerprint_signature: Option<String>,
    #[serde(rename = "OrgPublicKey", default)]
    organization_key: Option<String>,
    #[serde(rename = "PrivateIntent", default)]
    private_intent: bool,
}

impl Unprivatization {
    #[must_use]
    pub fn state(&self) -> UnprivatizationState {
        self.state
    }

    #[must_use]
    pub fn admin_email(&self) -> &str {
        &self.admin_email
    }

    #[must_use]
    pub fn invitation_email(&self) -> Option<&str> {
        self.invitation_email.as_deref()
    }

    #[must_use]
    pub fn has_private_intent(&self) -> bool {
        self.private_intent
    }
}

#[derive(Debug, Clone)]
pub struct VerifiedUnprivatization {
    organization_key: PublicKey,
    invitation_address: Option<String>,
    admin_email: String,
}

impl VerifiedUnprivatization {
    #[must_use]
    pub fn organization_key(&self) -> &PublicKey {
        &self.organization_key
    }

    #[must_use]
    pub fn invitation_address(&self) -> Option<&str> {
        self.invitation_address.as_deref()
    }

    #[must_use]
    pub fn admin_email(&self) -> &str {
        &self.admin_email
    }

    pub(crate) fn encrypt_token(&self, pgp: &Pgp, signer: &PrivateKey) -> Result<(Secret, String)> {
        let token = Secret::random_hex(TOKEN_LEN);
        let encrypted = pgp
            .encrypt()
            .to(self.organization_key.clone())
            .signed_by(signer.clone())
            .with_context(SignatureContext::new(TOKEN_CONTEXT).critical(true))
            .as_utf8()
            .armored_message(token.expose())?;

        Ok((token, encrypted))
    }
}

impl Account {
    pub async fn unprivatization(&self) -> Result<Unprivatization> {
        Ok(self.api().get(UNPRIVATIZE_PATH).send().await?)
    }

    pub async fn verify_unprivatization(
        &self,
        unprivatization: &Unprivatization,
    ) -> Result<Option<VerifiedUnprivatization>> {
        if unprivatization.private_intent {
            return Ok(None);
        }
        let armored = unprivatization
            .organization_key
            .as_deref()
            .ok_or(Error::Incomplete("OrgPublicKey"))?;
        let fingerprint_signature = unprivatization
            .fingerprint_signature
            .as_deref()
            .ok_or(Error::Incomplete("OrgKeyFingerprintSignature"))?;
        let organization_key = PublicKey::import(armored)?;

        let admin = self
            .public_keys(&unprivatization.admin_email)
            .send()
            .await?
            .into_public_keys()?;
        let admin_keys: Vec<PublicKey> = admin.address().verifying_keys().collect();
        let fingerprint = organization_key
            .sha256_fingerprints()
            .into_iter()
            .next()
            .ok_or(Error::Incomplete("OrgPublicKey"))?;

        let pgp = self.pgp();
        require_valid(
            pgp.verify()
                .with_keys(admin_keys)
                .with_context(VerificationContext::new(FINGERPRINT_CONTEXT))
                .as_utf8()
                .detached(fingerprint.as_str(), fingerprint_signature),
        )?;

        let invitation_address = match &unprivatization.invitation_data {
            None => None,
            Some(data) => {
                let signature = unprivatization
                    .invitation_signature
                    .as_deref()
                    .ok_or(Error::Incomplete("InvitationSignature"))?;
                require_valid(
                    pgp.verify()
                        .with_key(organization_key.clone())
                        .with_context(VerificationContext::new(INVITATION_CONTEXT))
                        .as_utf8()
                        .detached(data, signature),
                )?;
                let invitation: Invitation =
                    serde_json::from_str(data).map_err(|_| Error::MalformedInvitation)?;
                Some(invitation.address)
            }
        };

        Ok(Some(VerifiedUnprivatization {
            organization_key,
            invitation_address,
            admin_email: unprivatization.admin_email.clone(),
        }))
    }

    pub async fn accept_unprivatization(
        &self,
        keys: &AccountKeys,
        unprivatization: &VerifiedUnprivatization,
    ) -> Result<()> {
        let address = match unprivatization.invitation_address() {
            Some(email) => keys
                .address_by_email(email)
                .ok_or_else(|| Error::NoInvitationAddress(email.to_owned()))?,
            None => keys.primary_address()?,
        };
        let signer = keys
            .signing_key(address.id())
            .ok_or_else(|| Error::NoPrimaryAddressKey(address.id().clone()))?
            .clone();
        let user_keys: Vec<PrivateKey> = keys.user_keys().private_keys().to_vec();
        if user_keys.is_empty() {
            return Err(Error::NoUserKey);
        }

        let (token, activation_token) = unprivatization.encrypt_token(self.pgp(), &signer)?;
        let locked_keys: Vec<String> = parallel_map_each(user_keys, move |key| {
            Ok::<_, Error>(key.lock(token.expose())?.export_armored()?)
        })
        .await
        .into_iter()
        .collect::<Result<_>>()?;

        self.api()
            .post(UNPRIVATIZE_PATH)
            .json(&AcceptRequest {
                user_keys: locked_keys,
                activation_token,
            })
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn decline_unprivatization(&self) -> Result<()> {
        self.api().delete(UNPRIVATIZE_PATH).send_unit().await?;

        Ok(())
    }
}

fn require_valid(status: SignatureStatus) -> Result<()> {
    match status.is_valid() {
        true => Ok(()),
        false => Err(Error::UntrustedOrganization(Box::new(status))),
    }
}

#[derive(Debug, Deserialize)]
struct Invitation {
    #[serde(rename = "Address")]
    address: String,
}

#[derive(Debug, Serialize)]
struct AcceptRequest {
    #[serde(rename = "OrgUserKeys")]
    user_keys: Vec<String>,
    #[serde(rename = "OrgActivationToken")]
    activation_token: String,
}
