use std::collections::HashMap;
use std::fmt;

use rikka_proton_account::{AccountKeys, AddressId};
use rikka_proton_api::base64;
use rikka_proton_pgp::{
    KeyRing, LockedKey, Pgp, PrivateKey, PublicKey, Secret, SecretBytes, SessionKey,
};
use serde::Serialize;

use crate::bootstrap::{CalendarKey, CalendarSettings, Passphrase};
use crate::calendar::Member;
use crate::error::{Error, Result};
use crate::identifier::{CalendarId, PassphraseId};
use crate::opening::armored_session_key;

const KEY_NAME: &str = "Calendar key";
const PASSPHRASE_BYTES: usize = 32;

#[derive(Clone)]
pub struct UnlockedCalendar {
    calendar_id: CalendarId,
    member: Member,
    address_key: PrivateKey,
    address_ring: KeyRing,
    ring: KeyRing,
    primary: Option<PrivateKey>,
    passphrase_session: Option<SessionKey>,
    settings: CalendarSettings,
}

impl fmt::Debug for UnlockedCalendar {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UnlockedCalendar")
            .field("calendar_id", &self.calendar_id)
            .field("member", self.member.id())
            .field("keys", &self.ring.len())
            .field("primary", &self.primary.is_some())
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct CalendarMaterial<'a> {
    pub calendar_id: &'a CalendarId,
    pub members: &'a [Member],
    pub keys: &'a [CalendarKey],
    pub passphrases: &'a [Passphrase],
    pub settings: CalendarSettings,
}

impl UnlockedCalendar {
    pub fn unlock(
        pgp: &Pgp,
        account: &AccountKeys,
        material: CalendarMaterial<'_>,
    ) -> Result<UnlockedCalendar> {
        let member = material
            .members
            .iter()
            .find(|member| account.address_keys(member.address_id()).is_some())
            .cloned()
            .ok_or_else(|| Error::NotMember(material.calendar_id.clone()))?;
        let address_ring = account.address_ring(member.address_id())?.clone();
        let address_key = account.signer(member.address_id())?.clone();

        let mut unlocked_passphrases: HashMap<PassphraseId, (SecretBytes, String)> = HashMap::new();
        for passphrase in material.passphrases {
            let Some(member_passphrase) = passphrase.for_member(member.id()) else {
                continue;
            };
            match pgp
                .decrypt()
                .with_ring(&address_ring)
                .as_utf8()
                .message(member_passphrase.armored())
            {
                Ok(plaintext) => {
                    unlocked_passphrases.insert(
                        passphrase.id().clone(),
                        (
                            SecretBytes::new(plaintext.into_data()),
                            member_passphrase.armored().to_owned(),
                        ),
                    );
                }
                Err(error) => {
                    tracing::warn!(passphrase = %passphrase.id(), %error, "failed to unlock calendar passphrase");
                }
            }
        }
        if unlocked_passphrases.is_empty() {
            return Err(Error::NoMemberPassphrase(material.calendar_id.clone()));
        }

        let mut ring = KeyRing::new();
        let mut primary = None;
        let mut passphrase_session = None;
        for key in material.keys {
            let Some((secret, armored)) = unlocked_passphrases.get(key.passphrase_id()) else {
                continue;
            };
            let unlocked =
                LockedKey::import(key.armored()).and_then(|locked| locked.unlock(secret.expose()));
            match unlocked {
                Ok(unlocked) => {
                    if key.is_primary() {
                        primary = Some(unlocked.clone());
                        passphrase_session =
                            Some(armored_session_key(pgp, armored, &address_ring)?);
                    }
                    ring = ring.with_private(unlocked);
                }
                Err(error) => {
                    tracing::warn!(key = %key.id(), %error, "failed to unlock calendar key");
                }
            }
        }
        if ring.is_empty() {
            return Err(Error::NoCalendarKey(material.calendar_id.clone()));
        }

        Ok(UnlockedCalendar {
            calendar_id: material.calendar_id.clone(),
            member,
            address_key,
            address_ring,
            ring,
            primary,
            passphrase_session,
            settings: material.settings,
        })
    }

    #[must_use]
    pub fn calendar_id(&self) -> &CalendarId {
        &self.calendar_id
    }

    #[must_use]
    pub fn member(&self) -> &Member {
        &self.member
    }

    #[must_use]
    pub fn address_id(&self) -> &AddressId {
        self.member.address_id()
    }

    #[must_use]
    pub fn address_key(&self) -> &PrivateKey {
        &self.address_key
    }

    #[must_use]
    pub fn address_ring(&self) -> &KeyRing {
        &self.address_ring
    }

    #[must_use]
    pub fn keys(&self) -> &KeyRing {
        &self.ring
    }

    pub fn primary_key(&self) -> Result<&PrivateKey> {
        self.primary
            .as_ref()
            .ok_or_else(|| Error::NoPrimaryCalendarKey(self.calendar_id.clone()))
    }

    pub fn primary_public_key(&self) -> Result<PublicKey> {
        Ok(self.primary_key()?.public_key())
    }

    pub(crate) fn passphrase_session(&self) -> Result<&SessionKey> {
        self.passphrase_session
            .as_ref()
            .ok_or_else(|| Error::NoPrimaryCalendarKey(self.calendar_id.clone()))
    }

    #[must_use]
    pub fn settings(&self) -> &CalendarSettings {
        &self.settings
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PassphrasePackets {
    #[serde(rename = "DataPacket")]
    data_packet: String,
    #[serde(rename = "KeyPacket")]
    key_packet: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct KeySetup {
    #[serde(rename = "AddressID")]
    address_id: AddressId,
    #[serde(rename = "Signature")]
    signature: String,
    #[serde(rename = "PrivateKey")]
    private_key: String,
    #[serde(rename = "Passphrase")]
    passphrase: PassphrasePackets,
}

pub(crate) fn key_setup(
    pgp: &Pgp,
    address_id: &AddressId,
    address_key: &PrivateKey,
) -> Result<KeySetup> {
    let passphrase = Secret::random_base64(PASSPHRASE_BYTES);
    let locked = pgp
        .generate_key()
        .user_id(KEY_NAME, "")
        .generate_locked(passphrase.expose().as_bytes())?;
    let session = SessionKey::generate();
    let data_packet = pgp
        .encrypt()
        .with_session_key(session.clone())
        .as_utf8()
        .message(passphrase.expose().as_bytes())?;
    let key_packet = pgp
        .encrypt()
        .to(address_key.public_key())
        .key_packets(&session)?;
    let signature = passphrase_signature(pgp, passphrase.expose().as_bytes(), address_key)?;

    Ok(KeySetup {
        address_id: address_id.clone(),
        signature,
        private_key: locked.export_armored()?,
        passphrase: PassphrasePackets {
            data_packet: base64::encode(data_packet),
            key_packet: base64::encode(key_packet),
        },
    })
}

pub(crate) fn passphrase_signature(
    pgp: &Pgp,
    passphrase: &[u8],
    key: &PrivateKey,
) -> Result<String> {
    Ok(pgp
        .sign()
        .with_key(key.clone())
        .as_utf8()
        .armored_detached(passphrase)?)
}
