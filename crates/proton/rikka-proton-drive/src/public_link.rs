use std::fmt;

use rikka_proton_api::offload;
use rikka_proton_pgp::{PrivateKey, Secret, Srp, SrpChallenge};
use serde::{Deserialize, Serialize};

use crate::client::DriveClient;
use crate::crypto;
use crate::error::{Error, Result};
use crate::identifier::{LinkId, RevisionId, VolumeId};
use crate::link::LinkType;
use crate::role::Role;

const URL_SEGMENT: &str = "/urls/";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum AbuseCategory {
    #[serde(rename = "spam")]
    Spam,
    #[serde(rename = "copyright")]
    Copyright,
    #[serde(rename = "child-abuse")]
    ChildAbuse,
    #[serde(rename = "stolen-data")]
    StolenData,
    #[serde(rename = "malware")]
    Malware,
    #[serde(rename = "non-consensual-intimate")]
    NonConsensualIntimate,
    #[serde(rename = "other")]
    Other,
}

#[must_use]
#[derive(Debug, Clone)]
pub struct AbuseReport {
    category: AbuseCategory,
    reporter_email: Option<String>,
    message: Option<String>,
    revision: Option<RevisionId>,
}

impl AbuseReport {
    pub fn new(category: AbuseCategory) -> AbuseReport {
        AbuseReport {
            category,
            reporter_email: None,
            message: None,
            revision: None,
        }
    }

    pub fn reporter_email(mut self, email: impl Into<String>) -> Self {
        self.reporter_email = Some(email.into());
        self
    }

    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn revision(mut self, revision: impl Into<RevisionId>) -> Self {
        self.revision = Some(revision.into());
        self
    }
}

pub struct PublicLink {
    url: String,
    token: String,
    password: Secret,
    volume_id: VolumeId,
    link_id: LinkId,
    link_type: LinkType,
    media_type: String,
    role: Role,
    share_key: PrivateKey,
    root_passphrase: Secret,
    name: String,
}

impl PublicLink {
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
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
    pub fn media_type(&self) -> &str {
        &self.media_type
    }

    #[must_use]
    pub fn role(&self) -> Role {
        self.role
    }

    #[must_use]
    pub fn share_key(&self) -> &PrivateKey {
        &self.share_key
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Debug for PublicLink {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublicLink")
            .field("token", &self.token)
            .field("volume_id", &self.volume_id)
            .field("link_id", &self.link_id)
            .field("link_type", &self.link_type)
            .field("role", &self.role)
            .finish_non_exhaustive()
    }
}

impl DriveClient {
    pub async fn open_public_link(
        &self,
        url: &str,
        custom_password: Option<&str>,
    ) -> Result<PublicLink> {
        let (token, generated) = parse_public_url(url)?;
        let password = Secret::new(format!(
            "{generated}{}",
            custom_password.unwrap_or_default()
        ));

        let challenge: SrpChallenge = self
            .api()
            .get(format!("drive/urls/{token}/info"))
            .send()
            .await?;
        let srp_session = challenge.session().unwrap_or_default().to_owned();
        let proofs = {
            let password = password.clone();
            offload(move || challenge.proofs(None, password.expose())).await?
        };

        let auth: AuthResponse = self
            .api()
            .post(format!("drive/urls/{token}/auth"))
            .json(&AuthRequest {
                client_ephemeral: proofs.client_ephemeral(),
                client_proof: proofs.client_proof(),
                srp_session: &srp_session,
            })
            .send()
            .await?;
        proofs.verify_server_proof(&auth.server_proof)?;

        let bootstrap: TokenWire = self
            .api()
            .get(format!("drive/urls/{token}"))
            .anonymous()
            .header("x-pm-uid", auth.uid.as_str())
            .header("Authorization", format!("Bearer {}", auth.access_token))
            .send_field("Token")
            .await?;

        let client = self.clone();
        let url = url.to_owned();
        let token = token.to_owned();
        offload(move || client.decrypt_public_link(url, token, password, auth.share, bootstrap))
            .await
    }

    pub async fn report_public_link(&self, link: &PublicLink, report: AbuseReport) -> Result<()> {
        self.api()
            .post("drive/report/url")
            .json(&ReportRequest {
                share_url: &link.url,
                password: link.password.expose(),
                category: report.category,
                reporter_email: report.reporter_email.as_deref(),
                reporter_message: report.message.as_deref(),
                resource_passphrase: link.root_passphrase.expose(),
                bona_fide: true,
                volume_id: &link.volume_id,
                link_id: &link.link_id,
                revision_id: report.revision.as_ref(),
            })
            .send_unit()
            .await?;

        Ok(())
    }

    fn decrypt_public_link(
        &self,
        url: String,
        token: String,
        password: Secret,
        share: AuthShare,
        root: TokenWire,
    ) -> Result<PublicLink> {
        let salt = rikka_proton_api::base64::decode(&share.share_password_salt)
            .map_err(|_| Error::NotBase64("share password salt"))?;
        let key_password = Srp::new().mailbox_passphrase(password.expose(), salt)?;
        let unlocked_share_key = crypto::decrypt_key_with_password(
            self.pgp(),
            &share.share_key,
            &share.share_passphrase,
            key_password.as_bytes(),
        )?;
        let root_key = crypto::decrypt_key(
            self.pgp(),
            &root.node_key,
            &root.node_passphrase,
            None,
            std::slice::from_ref(&unlocked_share_key.key),
            &[],
        )?;
        let name = crypto::decrypt_name(self.pgp(), &unlocked_share_key.key, &[], &root.name)?.name;

        Ok(PublicLink {
            url,
            token,
            password,
            volume_id: share.volume_id,
            link_id: share.link_id,
            link_type: root.link_type,
            media_type: root.media_type,
            role: share.role,
            share_key: unlocked_share_key.key,
            root_passphrase: root_key.passphrase,
            name,
        })
    }
}

pub(crate) fn parse_public_url(url: &str) -> Result<(&str, &str)> {
    let refused = || Error::NotAPublicLink(url.to_owned());
    let (address, password) = url.split_once('#').ok_or_else(refused)?;
    let start = address.find(URL_SEGMENT).ok_or_else(refused)? + URL_SEGMENT.len();
    let token = address[start..]
        .split(['/', '?'])
        .next()
        .filter(|token| !token.is_empty())
        .ok_or_else(refused)?;
    if password.is_empty() {
        return Err(refused());
    }

    Ok((token, password))
}

#[derive(Debug, Serialize)]
struct AuthRequest<'a> {
    #[serde(rename = "ClientEphemeral")]
    client_ephemeral: &'a str,
    #[serde(rename = "ClientProof")]
    client_proof: &'a str,
    #[serde(rename = "SRPSession")]
    srp_session: &'a str,
}

#[derive(Debug, Deserialize)]
struct AuthResponse {
    #[serde(rename = "UID")]
    uid: String,
    #[serde(rename = "AccessToken")]
    access_token: String,
    #[serde(rename = "ServerProof")]
    server_proof: String,
    #[serde(rename = "Share")]
    share: AuthShare,
}

#[derive(Debug, Deserialize)]
struct AuthShare {
    #[serde(rename = "VolumeID")]
    volume_id: VolumeId,
    #[serde(rename = "LinkID")]
    link_id: LinkId,
    #[serde(rename = "SharePasswordSalt")]
    share_password_salt: String,
    #[serde(rename = "SharePassphrase")]
    share_passphrase: String,
    #[serde(rename = "ShareKey")]
    share_key: String,
    #[serde(rename = "PublicPermissions", default)]
    role: Role,
}

#[derive(Debug, Deserialize)]
struct TokenWire {
    #[serde(rename = "LinkType", default)]
    link_type: LinkType,
    #[serde(rename = "NodeKey")]
    node_key: String,
    #[serde(rename = "NodePassphrase")]
    node_passphrase: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "MIMEType", default)]
    media_type: String,
}

#[derive(Debug, Serialize)]
struct ReportRequest<'a> {
    #[serde(rename = "ShareURL")]
    share_url: &'a str,
    #[serde(rename = "Password")]
    password: &'a str,
    #[serde(rename = "AbuseCategory")]
    category: AbuseCategory,
    #[serde(rename = "ReporterEmail")]
    reporter_email: Option<&'a str>,
    #[serde(rename = "ReporterMessage")]
    reporter_message: Option<&'a str>,
    #[serde(rename = "ResourcePassphrase")]
    resource_passphrase: &'a str,
    #[serde(rename = "BonaFide")]
    bona_fide: bool,
    #[serde(rename = "VolumeID")]
    volume_id: &'a VolumeId,
    #[serde(rename = "LinkID")]
    link_id: &'a LinkId,
    #[serde(rename = "RevisionID")]
    revision_id: Option<&'a RevisionId>,
}
