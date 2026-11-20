use jiff::Timestamp;
use rikka_proton_account::{AddressId, KeyId};
use rikka_proton_api::offload;
use rikka_proton_pgp::{Secret, Srp};
use serde::{Deserialize, Serialize};

use crate::client::DriveClient;
use crate::crypto;
use crate::error::{Error, Result};
use crate::identifier::{LinkId, VolumeId};
use crate::link::LinkType;
use crate::public_link::parse_public_url;
use crate::sharing::ShareUrlFlags;
use crate::sharing::url_password::split;

const PUBLIC_ORIGIN: &str = "https://drive.proton.me/urls";

#[derive(Debug, Clone)]
pub struct Bookmark {
    token: String,
    volume_id: VolumeId,
    link_id: LinkId,
    link_type: LinkType,
    media_type: String,
    created_at: Option<Timestamp>,
    password: Secret,
    custom_password: Option<Secret>,
    name: String,
}

impl Bookmark {
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
    pub fn created_at(&self) -> Option<Timestamp> {
        self.created_at
    }

    #[must_use]
    pub fn url(&self) -> String {
        format!("{PUBLIC_ORIGIN}/{}#{}", self.token, self.password.expose())
    }

    #[must_use]
    pub fn custom_password(&self) -> Option<&Secret> {
        self.custom_password.as_ref()
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl DriveClient {
    pub async fn bookmarks(&self) -> Result<Vec<Result<Bookmark>>> {
        let bookmarks: Vec<BookmarkWire> = self
            .api()
            .get("drive/v2/shared-bookmarks")
            .send_field_or_default("Bookmarks")
            .await?;
        let client = self.clone();

        Ok(offload(move || {
            bookmarks
                .into_iter()
                .map(|wire| client.decrypt_bookmark(wire))
                .collect()
        })
        .await)
    }

    pub async fn save_bookmark(&self, url: &str, custom_password: Option<&str>) -> Result<()> {
        let (token, generated) = parse_public_url(url)?;
        let password = format!("{generated}{}", custom_password.unwrap_or_default());
        let address_id = self.primary_address()?;
        let (address_key_id, address_key) = crypto::primary_address_key(self.keys(), &address_id)?;
        let encrypted = crypto::encrypt_text(
            self.pgp(),
            &address_key.public_key(),
            Some(&address_key),
            &password,
        )?;

        self.api()
            .post(format!("drive/v2/urls/{token}/bookmark"))
            .json(&SaveRequest {
                bookmark: SavedBookmark {
                    encrypted_url_password: &encrypted,
                    address_id: &address_id,
                    address_key_id: &address_key_id,
                },
            })
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn delete_bookmark(&self, token: &str) -> Result<()> {
        self.api()
            .delete(format!("drive/v2/urls/{token}/bookmark"))
            .send_unit()
            .await?;

        Ok(())
    }

    fn decrypt_bookmark(&self, wire: BookmarkWire) -> Result<Bookmark> {
        let encrypted = wire
            .encrypted_url_password
            .ok_or(Error::Incomplete("bookmark password"))?;
        let address_id = self.primary_address()?;
        let address_keys = self.keys().address_ring(&address_id)?;
        let full = crypto::decrypt_text(
            self.pgp(),
            address_keys.private_keys(),
            &encrypted,
            "bookmark password",
        )?;
        let (password, custom_password) = split(&full, ShareUrlFlags::GeneratedAndCustomPassword);

        let token = wire.token;
        let salt = rikka_proton_api::base64::decode(&token.share_password_salt)
            .map_err(|_| Error::NotBase64("share password salt"))?;
        let key_password = Srp::new().mailbox_passphrase(&full, salt)?;
        let share = crypto::decrypt_key_with_password(
            self.pgp(),
            &token.share_key,
            &token.share_passphrase,
            key_password.as_bytes(),
        )?;
        let name = crypto::decrypt_name(self.pgp(), &share.key, &[], &token.name)?.name;

        Ok(Bookmark {
            token: token.token,
            volume_id: token.volume_id,
            link_id: token.link_id,
            link_type: token.link_type,
            media_type: token.media_type,
            created_at: wire.created_at,
            password,
            custom_password,
            name,
        })
    }
}

#[derive(Debug, Serialize)]
struct SaveRequest<'a> {
    #[serde(rename = "BookmarkShareURL")]
    bookmark: SavedBookmark<'a>,
}

#[derive(Debug, Serialize)]
struct SavedBookmark<'a> {
    #[serde(rename = "EncryptedUrlPassword")]
    encrypted_url_password: &'a str,
    #[serde(rename = "AddressID")]
    address_id: &'a AddressId,
    #[serde(rename = "AddressKeyID")]
    address_key_id: &'a KeyId,
}

#[derive(Debug, Deserialize)]
struct BookmarkWire {
    #[serde(rename = "EncryptedUrlPassword", default)]
    encrypted_url_password: Option<String>,
    #[serde(
        rename = "CreateTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    created_at: Option<Timestamp>,
    #[serde(rename = "Token")]
    token: TokenWire,
}

#[derive(Debug, Deserialize)]
struct TokenWire {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "LinkType", default)]
    link_type: LinkType,
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
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "MIMEType", default)]
    media_type: String,
}
