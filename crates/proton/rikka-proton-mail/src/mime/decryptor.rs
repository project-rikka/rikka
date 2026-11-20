use rikka_proton_pgp::{Pgp, PrivateKey, PublicKey, SignatureStatus};

use crate::mime::message::MimeMessage;
use crate::mime::parser::{self, DefaultContentType};
use crate::mime::part::{MimePart, PartBody, Protection};

const MAX_DEPTH: usize = 16;

pub(crate) enum Secret<'a> {
    Keys(&'a [PrivateKey]),
    Password(&'a [u8]),
}

pub(crate) struct MimeDecryptor<'a> {
    pgp: &'a Pgp,
    secret: Secret<'a>,
    verifiers: &'a [PublicKey],
}

impl<'a> MimeDecryptor<'a> {
    pub(crate) fn new(
        pgp: &'a Pgp,
        secret: Secret<'a>,
        verifiers: &'a [PublicKey],
    ) -> MimeDecryptor<'a> {
        MimeDecryptor {
            pgp,
            secret,
            verifiers,
        }
    }

    pub(crate) fn parse(&self, raw: &[u8]) -> MimeMessage {
        let mut message = MimeMessage::parse(raw);
        self.decrypt_parts(message.root_mut(), 0);
        message
    }

    fn decrypt_parts(&self, part: &mut MimePart, depth: usize) {
        if depth >= MAX_DEPTH {
            return;
        }

        let PartBody::Multipart(multipart) = &mut part.body else {
            return;
        };

        match &mut multipart.protection {
            Protection::Plain => {}
            Protection::Signed {
                signed,
                signature,
                status,
            } => *status = Some(self.verify(signed, signature)),
            Protection::Encrypted {
                payload,
                decrypted,
                status,
            } => {
                if let Some((data, signature)) = self.decrypt(payload) {
                    let mut inner = parser::parse(&data, DefaultContentType::Text, depth + 1);
                    self.decrypt_parts(&mut inner, depth + 1);
                    *decrypted = Some(Box::new(inner));
                    *status = Some(signature);
                }
            }
        }

        for child in &mut multipart.parts {
            self.decrypt_parts(child, depth + 1);
        }
    }

    fn verify(&self, signed: &[u8], signature: &[u8]) -> SignatureStatus {
        let binary = match rikka_proton_pgp::is_armored(signature) {
            true => match rikka_proton_pgp::unarmor(signature) {
                Ok(binary) => binary,
                Err(error) => {
                    return SignatureStatus::Invalid {
                        key_id: None,
                        reason: error.to_string(),
                    };
                }
            },
            false => signature.to_vec(),
        };

        self.pgp
            .verify()
            .with_keys(self.verifiers.to_vec())
            .detached(signed, binary)
    }

    fn decrypt(&self, payload: &[u8]) -> Option<(Vec<u8>, SignatureStatus)> {
        let decryption = self.pgp.decrypt();
        let decryption = match &self.secret {
            Secret::Keys(keys) => decryption.with_keys(keys.to_vec()),
            Secret::Password(password) => decryption.with_password(password),
        };

        let decrypted = decryption
            .verified_by_keys(self.verifiers.to_vec())
            .message(payload)
            .ok()?;
        let signature = decrypted.signature().clone();
        Some((decrypted.into_data(), signature))
    }
}
