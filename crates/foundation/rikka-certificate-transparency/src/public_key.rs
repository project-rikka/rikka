use std::fmt;

use rustls_pki_types::SignatureVerificationAlgorithm;
use webpki::aws_lc_rs::{ECDSA_P256_SHA256, RSA_PKCS1_2048_8192_SHA256};
use x509_cert::der::Decode as _;
use x509_cert::der::asn1::ObjectIdentifier;
use x509_cert::spki::SubjectPublicKeyInfoRef;

const EC_PUBLIC_KEY: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.2.1");
const P256: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.3.1.7");
const RSA_ENCRYPTION: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.1");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyKind {
    Rsa,
    EcdsaP256,
}

impl KeyKind {
    pub(crate) fn tls_signature_algorithm(self) -> u8 {
        match self {
            KeyKind::Rsa => 1,
            KeyKind::EcdsaP256 => 3,
        }
    }

    fn algorithm(self) -> &'static dyn SignatureVerificationAlgorithm {
        match self {
            KeyKind::Rsa => RSA_PKCS1_2048_8192_SHA256,
            KeyKind::EcdsaP256 => ECDSA_P256_SHA256,
        }
    }
}

#[derive(Clone)]
pub(crate) struct PublicKey {
    kind: KeyKind,
    subject_public_key: Vec<u8>,
}

impl PublicKey {
    pub(crate) fn from_spki(der: &[u8]) -> Option<PublicKey> {
        let info = SubjectPublicKeyInfoRef::from_der(der).ok()?;
        let kind = match info.algorithm.oid {
            EC_PUBLIC_KEY => {
                let curve = info
                    .algorithm
                    .parameters?
                    .decode_as::<ObjectIdentifier>()
                    .ok()?;
                (curve == P256).then_some(KeyKind::EcdsaP256)?
            }
            RSA_ENCRYPTION => KeyKind::Rsa,
            _ => return None,
        };
        Some(PublicKey {
            kind,
            subject_public_key: info.subject_public_key.as_bytes()?.to_vec(),
        })
    }

    pub(crate) fn kind(&self) -> KeyKind {
        self.kind
    }

    pub(crate) fn verifies(&self, message: &[u8], signature: &[u8]) -> bool {
        self.kind
            .algorithm()
            .verify_signature(&self.subject_public_key, message, signature)
            .is_ok()
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublicKey")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}
