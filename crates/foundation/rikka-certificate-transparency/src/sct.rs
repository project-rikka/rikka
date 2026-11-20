use jiff::Timestamp;
use sha2::{Digest as _, Sha256};
use x509_cert::Certificate;
use x509_cert::der::asn1::{ObjectIdentifier, OctetString};
use x509_cert::der::{Decode as _, Encode as _};

use crate::error::{Error, Result};
use crate::list::LogList;
use crate::log::{Log, LogId, Standing};

const SCT_LIST: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.3.6.1.4.1.11129.2.4.2");
const SCT_VERSION_V1: u8 = 0;
const CERTIFICATE_TIMESTAMP: u8 = 0;
const PRECERT_ENTRY: [u8; 2] = [0, 1];
const HASH_SHA256: u8 = 4;
const MAX_TBS_LENGTH: usize = (1 << 24) - 1;

#[derive(Debug, Clone, Copy)]
pub struct ValidSct<'a> {
    log: &'a Log,
    timestamp: Timestamp,
    standing: Standing,
}

impl<'a> ValidSct<'a> {
    #[must_use]
    pub fn log(&self) -> &'a Log {
        self.log
    }

    #[must_use]
    pub fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    #[must_use]
    pub fn standing(&self) -> Standing {
        self.standing
    }
}

struct SignedCertificateTimestamp<'a> {
    log_id: [u8; 32],
    time: &'a [u8],
    extensions: &'a [u8],
    hash_algorithm: u8,
    signature_algorithm: u8,
    signature: &'a [u8],
}

impl LogList {
    pub fn embedded_scts(
        &self,
        leaf: &[u8],
        issuer_public_key_info: &[u8],
    ) -> Result<Vec<ValidSct<'_>>> {
        let leaf = Certificate::from_der(leaf)
            .map_err(|error| Error::MalformedCertificate(error.to_string()))?;
        let Some(extension) = leaf
            .tbs_certificate
            .extensions
            .iter()
            .flatten()
            .find(|extension| extension.extn_id == SCT_LIST)
        else {
            return Ok(Vec::new());
        };
        let sct_list = OctetString::from_der(extension.extn_value.as_bytes())
            .map_err(|_| Error::MalformedSctList)?;
        let precertificate = precertificate_tbs(&leaf)?;
        let issuer_key_hash: [u8; 32] = Sha256::digest(issuer_public_key_info).into();

        let signed: Vec<(&Log, Timestamp)> = parse_sct_list(sct_list.as_bytes())?
            .into_iter()
            .flatten()
            .filter_map(|sct| {
                let log = self
                    .log(&LogId::new(sct.log_id))
                    .filter(|log| log.state.was_qualified())?;
                let timestamp = sct_time(sct.time)?;
                let algorithms_match = sct.hash_algorithm == HASH_SHA256
                    && sct.signature_algorithm == log.key.kind().tls_signature_algorithm();
                let message = signed_data(&sct, &issuer_key_hash, &precertificate);
                (algorithms_match && log.key.verifies(&message, sct.signature))
                    .then_some((log, timestamp))
            })
            .collect();

        let Some(earliest) = signed.iter().map(|(_, timestamp)| *timestamp).min() else {
            return Ok(Vec::new());
        };
        Ok(signed
            .into_iter()
            .filter_map(|(log, timestamp)| {
                let standing = log.state.standing(earliest)?;
                Some(ValidSct {
                    log,
                    timestamp,
                    standing,
                })
            })
            .collect())
    }
}

fn precertificate_tbs(leaf: &Certificate) -> Result<Vec<u8>> {
    let mut tbs = leaf.tbs_certificate.clone();
    let extensions = tbs.extensions.take().unwrap_or_default();
    let retained: Vec<_> = extensions
        .into_iter()
        .filter(|extension| extension.extn_id != SCT_LIST)
        .collect();
    tbs.extensions = (!retained.is_empty()).then_some(retained);
    let tbs = tbs
        .to_der()
        .map_err(|error| Error::MalformedCertificate(error.to_string()))?;
    if tbs.len() > MAX_TBS_LENGTH {
        return Err(Error::MalformedCertificate(format!(
            "TBS certificate of {} bytes exceeds a 24-bit length",
            tbs.len()
        )));
    }
    Ok(tbs)
}

fn sct_time(time: &[u8]) -> Option<Timestamp> {
    let milliseconds = u64::from_be_bytes(time.try_into().ok()?);
    Timestamp::from_millisecond(i64::try_from(milliseconds).ok()?).ok()
}

fn signed_data(
    sct: &SignedCertificateTimestamp<'_>,
    issuer_key_hash: &[u8; 32],
    tbs: &[u8],
) -> Vec<u8> {
    let tbs_length = u32::try_from(tbs.len())
        .expect("precertificate length is checked to fit 24 bits")
        .to_be_bytes();
    let extensions_length = u16::try_from(sct.extensions.len())
        .expect("SCT extensions are read from a u16-prefixed field")
        .to_be_bytes();

    let mut signed = Vec::with_capacity(tbs.len() + sct.extensions.len() + 52);
    signed.push(SCT_VERSION_V1);
    signed.push(CERTIFICATE_TIMESTAMP);
    signed.extend_from_slice(sct.time);
    signed.extend_from_slice(&PRECERT_ENTRY);
    signed.extend_from_slice(issuer_key_hash);
    signed.extend_from_slice(&tbs_length[1..]);
    signed.extend_from_slice(tbs);
    signed.extend_from_slice(&extensions_length);
    signed.extend_from_slice(sct.extensions);
    signed
}

fn parse_sct_list(encoded: &[u8]) -> Result<Vec<Option<SignedCertificateTimestamp<'_>>>> {
    let mut list = Reader(encoded);
    let mut entries = Reader(list.prefixed_u16().ok_or(Error::MalformedSctList)?);
    if !list.0.is_empty() {
        return Err(Error::MalformedSctList);
    }
    let mut scts = Vec::new();
    while !entries.0.is_empty() {
        scts.push(parse_sct(
            entries.prefixed_u16().ok_or(Error::MalformedSctList)?,
        ));
    }
    Ok(scts)
}

fn parse_sct(encoded: &[u8]) -> Option<SignedCertificateTimestamp<'_>> {
    let mut reader = Reader(encoded);
    if reader.take(1)? != [SCT_VERSION_V1] {
        return None;
    }
    let log_id = reader.take(32)?.try_into().ok()?;
    let time = reader.take(8)?;
    let extensions = reader.prefixed_u16()?;
    let hash_algorithm = reader.take(1)?[0];
    let signature_algorithm = reader.take(1)?[0];
    let signature = reader.prefixed_u16()?;
    reader.0.is_empty().then_some(SignedCertificateTimestamp {
        log_id,
        time,
        extensions,
        hash_algorithm,
        signature_algorithm,
        signature,
    })
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        if self.0.len() < count {
            return None;
        }
        let (taken, rest) = self.0.split_at(count);
        self.0 = rest;
        Some(taken)
    }

    fn prefixed_u16(&mut self) -> Option<&'a [u8]> {
        let length = self.take(2)?;
        self.take(usize::from(u16::from_be_bytes([length[0], length[1]])))
    }
}
