use rikka_proton_api::percent::encode;

use crate::code::{Algorithm, Codes, MAX_DIGITS};
use crate::entry::hotp::Hotp;
use crate::entry::non_empty;
use crate::entry::steam::Steam;
use crate::entry::totp::Totp;
use crate::entry::uri::{Uri, decode_component};
use crate::error::{Error, Result};

const OTPAUTH: &str = "otpauth";
const STEAM: &str = "steam";
const TOTP: &str = "totp";
const HOTP: &str = "hotp";
const NULL_VALUE: &str = "null";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Otp {
    Totp(Totp),
    Hotp(Hotp),
    Steam(Steam),
}

impl Otp {
    pub fn parse(text: &str) -> Result<Otp> {
        let uri = Uri::parse(text)?;
        match uri.scheme() {
            STEAM => Ok(Otp::Steam(Steam::new(uri.body()))),
            OTPAUTH => parse_otpauth(&uri),
            scheme => Err(Error::UnsupportedScheme {
                scheme: scheme.to_owned(),
            }),
        }
    }

    pub fn to_uri(&self) -> String {
        match self {
            Otp::Totp(totp) => format!(
                "{}&algorithm={}&digits={}&period={}",
                otpauth_prefix(
                    TOTP,
                    totp.label.as_deref(),
                    &totp.secret,
                    totp.issuer.as_deref()
                ),
                totp.algorithm.as_str(),
                totp.digits,
                totp.period
            ),
            Otp::Hotp(hotp) => format!(
                "{}&algorithm={}&digits={}&counter={}",
                otpauth_prefix(
                    HOTP,
                    hotp.label.as_deref(),
                    &hotp.secret,
                    hotp.issuer.as_deref()
                ),
                hotp.algorithm.as_str(),
                hotp.digits,
                hotp.counter
            ),
            Otp::Steam(Steam {
                secret,
                name: Some(name),
            }) => otpauth_prefix(STEAM, Some(name), secret, None),
            Otp::Steam(steam) => format!("{STEAM}://{}", steam.secret),
        }
    }

    pub fn label(&self) -> Option<&str> {
        match self {
            Otp::Totp(totp) => totp.label.as_deref(),
            Otp::Hotp(hotp) => hotp.label.as_deref(),
            Otp::Steam(steam) => steam.name.as_deref(),
        }
    }

    pub fn issuer(&self) -> Option<&str> {
        match self {
            Otp::Totp(totp) => totp.issuer.as_deref(),
            Otp::Hotp(hotp) => hotp.issuer.as_deref(),
            Otp::Steam(_) => Some(Steam::ISSUER),
        }
    }

    pub fn secret(&self) -> &str {
        match self {
            Otp::Totp(totp) => &totp.secret,
            Otp::Hotp(hotp) => &hotp.secret,
            Otp::Steam(steam) => &steam.secret,
        }
    }

    pub fn algorithm(&self) -> Algorithm {
        match self {
            Otp::Totp(totp) => totp.algorithm,
            Otp::Hotp(hotp) => hotp.algorithm,
            Otp::Steam(_) => Algorithm::Sha1,
        }
    }

    pub fn digits(&self) -> u8 {
        match self {
            Otp::Totp(totp) => totp.digits,
            Otp::Hotp(hotp) => hotp.digits,
            Otp::Steam(_) => Steam::DIGITS,
        }
    }

    pub fn period(&self) -> Option<u16> {
        match self {
            Otp::Totp(totp) => Some(totp.period),
            Otp::Hotp(_) => None,
            Otp::Steam(_) => Some(Steam::PERIOD),
        }
    }

    pub fn secret_bytes(&self) -> Result<Vec<u8>> {
        match self {
            Otp::Totp(totp) => totp.secret_bytes(),
            Otp::Hotp(hotp) => hotp.secret_bytes(),
            Otp::Steam(steam) => steam.secret_bytes(),
        }
    }

    pub fn remaining_at(&self, at: jiff::Timestamp) -> Option<u16> {
        match self {
            Otp::Totp(totp) => Some(totp.remaining_at(at)),
            Otp::Hotp(_) => None,
            Otp::Steam(steam) => Some(steam.remaining_at(at)),
        }
    }

    pub fn code_at(&self, at: jiff::Timestamp) -> Result<String> {
        match self {
            Otp::Totp(totp) => totp.code_at(at),
            Otp::Hotp(hotp) => hotp.code(),
            Otp::Steam(steam) => steam.code_at(at),
        }
    }

    pub fn codes_at(&self, at: jiff::Timestamp) -> Result<Codes> {
        match self {
            Otp::Totp(totp) => totp.codes_at(at),
            Otp::Hotp(hotp) => hotp.codes(),
            Otp::Steam(steam) => steam.codes_at(at),
        }
    }
}

fn parse_otpauth(uri: &Uri) -> Result<Otp> {
    let otp_type = uri.authority().to_ascii_lowercase();
    let (label, issuer) = label_and_issuer(uri);
    match otp_type.as_str() {
        TOTP => Ok(Otp::Totp(Totp {
            label,
            issuer,
            secret: secret(uri)?,
            algorithm: algorithm(uri)?,
            digits: digits(uri, Totp::DIGITS)?,
            period: period(uri)?,
        })),
        HOTP => Ok(Otp::Hotp(Hotp {
            label,
            issuer,
            secret: secret(uri)?,
            algorithm: algorithm(uri)?,
            digits: digits(uri, Hotp::DIGITS)?,
            counter: counter(uri)?,
        })),
        STEAM => Ok(Otp::Steam(Steam {
            secret: secret(uri)?,
            name: label,
        })),
        other => Err(Error::UnsupportedType {
            kind: other.to_owned(),
        }),
    }
}

fn otpauth_prefix(
    otp_type: &str,
    label: Option<&str>,
    secret: &str,
    issuer: Option<&str>,
) -> String {
    let label = label.map(encode).unwrap_or_default();
    let issuer = issuer
        .map(|issuer| format!("&issuer={}", encode(issuer)))
        .unwrap_or_default();
    format!(
        "{OTPAUTH}://{otp_type}/{label}?secret={}{issuer}",
        encode(secret)
    )
}

fn label_and_issuer(uri: &Uri) -> (Option<String>, Option<String>) {
    let issuer_param = uri.query_param("issuer").map(str::trim).and_then(non_empty);
    let Some(path) = uri.path() else {
        return (None, issuer_param);
    };
    let whole_label = || non_empty(decode_component(path).trim());
    let Some((prefix, account)) = path.split_once(':') else {
        return (whole_label(), issuer_param);
    };
    let label_issuer = decode_component(prefix).trim().to_owned();
    let account = decode_component(account).trim().to_owned();
    match issuer_param {
        Some(issuer) if issuer != label_issuer => (whole_label(), Some(issuer)),
        Some(issuer) => (non_empty(&account), Some(issuer)),
        None => (non_empty(&account), non_empty(&label_issuer)),
    }
}

fn secret(uri: &Uri) -> Result<String> {
    let secret = uri.query_param("secret").ok_or(Error::MissingSecret)?;
    if secret.is_empty() {
        return Err(Error::EmptySecret);
    }
    Ok(secret.to_owned())
}

fn algorithm(uri: &Uri) -> Result<Algorithm> {
    match param(uri, "algorithm") {
        Some(name) => name.parse(),
        None => Ok(Algorithm::default()),
    }
}

fn digits(uri: &Uri, default: u8) -> Result<u8> {
    let Some(value) = param(uri, "digits") else {
        return Ok(default);
    };
    match value.parse::<u8>() {
        Ok(digits) if (1..=MAX_DIGITS).contains(&digits) => Ok(digits),
        _ => Err(Error::InvalidDigits {
            digits: value.to_owned(),
        }),
    }
}

fn period(uri: &Uri) -> Result<u16> {
    let Some(value) = param(uri, "period") else {
        return Ok(Totp::PERIOD);
    };
    match value.parse::<u16>() {
        Ok(period) if period > 0 => Ok(period),
        _ => Err(Error::InvalidPeriod {
            period: value.to_owned(),
        }),
    }
}

fn counter(uri: &Uri) -> Result<u64> {
    let Some(value) = param(uri, "counter") else {
        return Ok(0);
    };
    value.parse::<u64>().map_err(|_| Error::InvalidCounter {
        counter: value.to_owned(),
    })
}

fn param<'a>(uri: &'a Uri, key: &str) -> Option<&'a str> {
    uri.query_param(key)
        .filter(|value| !value.is_empty() && *value != NULL_VALUE)
}
