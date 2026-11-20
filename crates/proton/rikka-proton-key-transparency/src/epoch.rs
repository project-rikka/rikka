use rikka_certificate_transparency::LogList;
use rikka_proton_api::wire_enum;
use rikka_proton_pgp::sha256;
use serde::{Deserialize, Serialize};

use crate::certificate::EpochCertificate;
use crate::failure::Failure;
use crate::merkle::{self, HASH_LENGTH};
use crate::realm::Realm;

const CHAIN_VERSION: u32 = 1;

wire_enum! {
    pub enum CertificateIssuer {
        LetsEncrypt = 0,
        ZeroSsl = 1,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Epoch {
    #[serde(rename = "EpochID")]
    id: u32,
    #[serde(rename = "TreeHash")]
    tree_hash: String,
    #[serde(rename = "ChainHash")]
    chain_hash: String,
    #[serde(rename = "PrevChainHash", default)]
    previous_chain_hash: String,
    #[serde(rename = "Certificate")]
    certificate: String,
    #[serde(rename = "CertificateIssuer")]
    certificate_issuer: CertificateIssuer,
    #[serde(rename = "CertificateTime")]
    certificate_time: u64,
    #[serde(rename = "ClaimedTime", default)]
    claimed_time: u64,
}

impl Epoch {
    #[must_use]
    pub fn id(&self) -> u32 {
        self.id
    }

    #[must_use]
    pub fn tree_hash(&self) -> &str {
        &self.tree_hash
    }

    #[must_use]
    pub fn chain_hash(&self) -> &str {
        &self.chain_hash
    }

    #[must_use]
    pub fn previous_chain_hash(&self) -> &str {
        &self.previous_chain_hash
    }

    #[must_use]
    pub fn certificate(&self) -> &str {
        &self.certificate
    }

    #[must_use]
    pub fn certificate_issuer(&self) -> CertificateIssuer {
        self.certificate_issuer
    }

    #[must_use]
    pub fn certificate_time(&self) -> u64 {
        self.certificate_time
    }

    #[must_use]
    pub fn claimed_time(&self) -> u64 {
        self.claimed_time
    }

    pub fn verify(&self, realm: &Realm, log_list: &LogList, time: u64) -> Result<u64, Failure> {
        self.verify_chain_hash()?;
        let certificate = EpochCertificate::parse(&self.certificate)?;
        let expected = self.alternative_name(realm);
        let found = certificate.alternative_name();
        if found.as_deref() != Some(expected.as_str()) {
            return Err(Failure::AlternativeNameMismatch { expected, found });
        }
        certificate.verify(self.certificate_issuer, log_list, time)?;
        Ok(certificate.not_before())
    }

    fn verify_chain_hash(&self) -> Result<(), Failure> {
        let tree = merkle::decode_hash(&self.tree_hash).ok_or(Failure::MalformedEpoch)?;
        let chain = merkle::decode_hash(&self.chain_hash).ok_or(Failure::MalformedEpoch)?;
        let mut input = rikka_types::hex::decode(&self.previous_chain_hash)
            .map_err(|_| Failure::MalformedEpoch)?;
        input.extend_from_slice(&tree);
        if sha256(&input) == chain {
            Ok(())
        } else {
            Err(Failure::ChainHashMismatch)
        }
    }

    fn alternative_name(&self, realm: &Realm) -> String {
        let (first, second) = self.chain_hash.split_at(HASH_LENGTH);
        format!(
            "{first}.{second}.{}.{}.{CHAIN_VERSION}.{}",
            self.certificate_time,
            self.id,
            realm.domain()
        )
    }
}
