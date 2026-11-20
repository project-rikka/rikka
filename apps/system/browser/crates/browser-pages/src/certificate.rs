use base64::Engine;
use x509_cert::der::Decode;

const BEGIN: &str = "-----BEGIN CERTIFICATE-----";
const END: &str = "-----END CERTIFICATE-----";
const LINE: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Certificate {
    pub pem: String,
    pub subject: Option<String>,
    pub issuer: Option<String>,
    pub valid_from: Option<i64>,
    pub valid_until: Option<i64>,
}

fn der_of(encoded: &str) -> Option<Vec<u8>> {
    let body: String = encoded
        .replace(BEGIN, "")
        .replace(END, "")
        .chars()
        .filter(|char| !char.is_whitespace())
        .collect();
    base64::engine::general_purpose::STANDARD.decode(body).ok()
}

fn pem_of(der: &[u8]) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(der);
    let lines: Vec<&str> = encoded
        .as_bytes()
        .chunks(LINE)
        .filter_map(|line| std::str::from_utf8(line).ok())
        .collect();
    format!("{BEGIN}\n{}\n{END}\n", lines.join("\n"))
}

fn seconds(duration: std::time::Duration) -> Option<i64> {
    i64::try_from(duration.as_secs()).ok()
}

impl Certificate {
    #[must_use]
    pub fn parse(encoded: &str) -> Option<Certificate> {
        let der = der_of(encoded)?;
        let parsed = x509_cert::Certificate::from_der(&der).ok();
        let tbs = parsed.as_ref().map(|parsed| &parsed.tbs_certificate);
        Some(Certificate {
            pem: pem_of(&der),
            subject: tbs.map(|tbs| tbs.subject.to_string()),
            issuer: tbs.map(|tbs| tbs.issuer.to_string()),
            valid_from: tbs.and_then(|tbs| seconds(tbs.validity.not_before.to_unix_duration())),
            valid_until: tbs.and_then(|tbs| seconds(tbs.validity.not_after.to_unix_duration())),
        })
    }
}
