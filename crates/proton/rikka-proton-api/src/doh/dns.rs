use crate::base32;
use thiserror::Error;

const ROUTING_DOMAIN: &str = "protonpro.xyz";
const TYPE_TXT: u16 = 16;
const CLASS_IN: u16 = 1;
const FLAG_RECURSION_DESIRED: u16 = 0x0100;
const RCODE_MASK: u8 = 0x0f;
const RCODE_NO_ERROR: u8 = 0;
const RCODE_NXDOMAIN: u8 = 3;
const POINTER_MASK: u8 = 0b1100_0000;
const HEADER_LEN: usize = 12;
const QUESTION_TRAILER_LEN: usize = 4;
const RECORD_HEADER_LEN: usize = 10;
const MAX_LABEL_LEN: usize = 63;
const MAX_HOSTNAME_LEN: usize = 255;

#[derive(Debug, PartialEq, Eq, Error)]
pub(crate) enum DnsError {
    #[error("truncated DNS response")]
    Truncated,
    #[error("DNS response code {0}")]
    ResponseCode(u8),
    #[error("NXDOMAIN")]
    NxDomain,
    #[error("invalid hostname {0:?} in TXT record")]
    InvalidHostname(String),
}

pub(crate) fn alternative_name(host: &str, session: Option<&str>) -> String {
    let name = format!("d{}.{ROUTING_DOMAIN}", base32::encode(host));
    match session.filter(|session| is_label(session)) {
        Some(session) => format!("{session}.{name}"),
        None => name,
    }
}

pub(crate) fn txt_query(name: &str) -> Vec<u8> {
    let mut query = Vec::with_capacity(HEADER_LEN + name.len() + 2 + QUESTION_TRAILER_LEN);

    let id = 0u16;
    let question_count = 1u16;
    for field in [id, FLAG_RECURSION_DESIRED, question_count, 0, 0, 0] {
        query.extend_from_slice(&field.to_be_bytes());
    }

    for label in name.split('.').filter(|label| !label.is_empty()) {
        let length = u8::try_from(label.len()).unwrap_or(u8::MAX);
        query.push(length);
        query.extend_from_slice(&label.as_bytes()[..usize::from(length)]);
    }
    query.push(0);

    query.extend_from_slice(&TYPE_TXT.to_be_bytes());
    query.extend_from_slice(&CLASS_IN.to_be_bytes());

    query
}

pub(crate) fn txt_hostnames(response: &[u8]) -> Result<Vec<String>, DnsError> {
    if response.len() < HEADER_LEN {
        return Err(DnsError::Truncated);
    }

    match response[3] & RCODE_MASK {
        RCODE_NO_ERROR => {}
        RCODE_NXDOMAIN => return Err(DnsError::NxDomain),
        code => return Err(DnsError::ResponseCode(code)),
    }

    let question_count = u16::from_be_bytes([response[4], response[5]]);
    let answer_count = u16::from_be_bytes([response[6], response[7]]);

    let mut offset = HEADER_LEN;
    for _ in 0..question_count {
        offset = skip_name(response, offset)? + QUESTION_TRAILER_LEN;
    }

    let mut hostnames = Vec::new();
    for _ in 0..answer_count {
        offset = skip_name(response, offset)?;

        let header = response
            .get(offset..offset + RECORD_HEADER_LEN)
            .ok_or(DnsError::Truncated)?;
        let record_type = u16::from_be_bytes([header[0], header[1]]);
        let class = u16::from_be_bytes([header[2], header[3]]);
        let data_len = usize::from(u16::from_be_bytes([header[8], header[9]]));
        offset += RECORD_HEADER_LEN;

        let data = response
            .get(offset..offset + data_len)
            .ok_or(DnsError::Truncated)?;
        offset += data_len;

        if record_type != TYPE_TXT || class != CLASS_IN {
            continue;
        }

        hostnames.push(txt_hostname(data)?);
    }

    Ok(hostnames)
}

fn txt_hostname(record: &[u8]) -> Result<String, DnsError> {
    let mut text = Vec::new();
    let mut offset = 0;

    while offset < record.len() {
        let length = usize::from(record[offset]);
        let part = record
            .get(offset + 1..offset + 1 + length)
            .ok_or(DnsError::Truncated)?;
        text.extend_from_slice(part);
        offset += 1 + length;
    }

    match String::from_utf8(text) {
        Ok(text) if is_hostname(&text) => Ok(text),
        Ok(text) => Err(DnsError::InvalidHostname(text)),
        Err(error) => Err(DnsError::InvalidHostname(
            String::from_utf8_lossy(error.as_bytes()).into_owned(),
        )),
    }
}

fn is_hostname(text: &str) -> bool {
    let trimmed = text.strip_suffix('.').unwrap_or(text);

    if trimmed.is_empty() || trimmed.len() > MAX_HOSTNAME_LEN {
        return false;
    }

    trimmed.split('.').all(is_label)
}

fn is_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= MAX_LABEL_LEN
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn skip_name(response: &[u8], mut offset: usize) -> Result<usize, DnsError> {
    loop {
        let length = *response.get(offset).ok_or(DnsError::Truncated)?;

        if length & POINTER_MASK == POINTER_MASK {
            response.get(offset + 1).ok_or(DnsError::Truncated)?;
            return Ok(offset + 2);
        }

        if length == 0 {
            return Ok(offset + 1);
        }

        offset += 1 + usize::from(length);
    }
}
