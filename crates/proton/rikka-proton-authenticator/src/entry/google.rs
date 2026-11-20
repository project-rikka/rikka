use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};

use crate::base32::Base32;
use crate::code::Algorithm;
use crate::entry::hotp::Hotp;
use crate::entry::non_empty;
use crate::entry::otp::Otp;
use crate::entry::totp::Totp;
use crate::entry::uri::Uri;
use crate::error::{Error, Result};

const MIGRATION_SCHEME: &str = "otpauth-migration";
const MIGRATION_PERIOD: u16 = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum MigrationAlgorithm {
    Unspecified = 0,
    Sha1 = 1,
    Sha256 = 2,
    Sha512 = 3,
    Md5 = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum DigitCount {
    Unspecified = 0,
    Six = 1,
    Eight = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum OtpType {
    Unspecified = 0,
    Hotp = 1,
    Totp = 2,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct OtpParameters {
    #[prost(bytes = "vec", tag = "1")]
    pub secret: Vec<u8>,
    #[prost(string, tag = "2")]
    pub name: String,
    #[prost(string, tag = "3")]
    pub issuer: String,
    #[prost(enumeration = "MigrationAlgorithm", tag = "4")]
    pub algorithm: i32,
    #[prost(enumeration = "DigitCount", tag = "5")]
    pub digits: i32,
    #[prost(enumeration = "OtpType", tag = "6")]
    pub kind: i32,
    #[prost(int64, optional, tag = "7")]
    pub counter: Option<i64>,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct MigrationPayload {
    #[prost(message, repeated, tag = "1")]
    pub otp_parameters: Vec<OtpParameters>,
    #[prost(int32, tag = "2")]
    pub version: i32,
    #[prost(int32, tag = "3")]
    pub batch_size: i32,
    #[prost(int32, optional, tag = "4")]
    pub batch_index: Option<i32>,
    #[prost(int32, tag = "5")]
    pub batch_id: i32,
}

impl MigrationPayload {
    pub fn decode(bytes: &[u8]) -> Result<MigrationPayload> {
        <MigrationPayload as prost::Message>::decode(bytes)
            .map_err(|error| Error::malformed("migration payload", error))
    }

    pub fn from_uri(text: &str) -> Result<MigrationPayload> {
        let uri = Uri::parse(text)?;
        if uri.scheme() != MIGRATION_SCHEME {
            return Err(Error::UnsupportedScheme {
                scheme: uri.scheme().to_owned(),
            });
        }
        let data = uri
            .query_param("data")
            .ok_or(Error::MalformedPayload)?
            .replace(' ', "+");
        let bytes = STANDARD
            .decode(&data)
            .or_else(|_| URL_SAFE_NO_PAD.decode(&data))
            .map_err(|_| Error::MalformedPayload)?;
        MigrationPayload::decode(&bytes)
    }

    pub fn otps(&self) -> Vec<Result<Otp>> {
        self.otp_parameters.iter().map(OtpParameters::otp).collect()
    }
}

impl OtpParameters {
    pub fn otp(&self) -> Result<Otp> {
        let secret = Base32::encode(&self.secret);
        let label = non_empty(&self.name);
        let issuer = non_empty(&self.issuer);
        let algorithm = Algorithm::try_from(self.algorithm())?;
        let digits = self.digits().count();
        match self.kind() {
            OtpType::Totp => Ok(Otp::Totp(Totp {
                label,
                issuer,
                secret,
                algorithm,
                digits,
                period: MIGRATION_PERIOD,
            })),
            OtpType::Hotp => Ok(Otp::Hotp(Hotp {
                label,
                issuer,
                secret,
                algorithm,
                digits,
                counter: self.counter.unwrap_or_default().max(0) as u64,
            })),
            OtpType::Unspecified => Err(Error::UnsupportedType {
                kind: "unspecified".to_owned(),
            }),
        }
    }
}

impl TryFrom<MigrationAlgorithm> for Algorithm {
    type Error = Error;

    fn try_from(algorithm: MigrationAlgorithm) -> Result<Algorithm> {
        match algorithm {
            MigrationAlgorithm::Unspecified | MigrationAlgorithm::Sha1 => Ok(Algorithm::Sha1),
            MigrationAlgorithm::Sha256 => Ok(Algorithm::Sha256),
            MigrationAlgorithm::Sha512 => Ok(Algorithm::Sha512),
            MigrationAlgorithm::Md5 => Err(Error::UnsupportedAlgorithm {
                name: "MD5".to_owned(),
            }),
        }
    }
}

impl DigitCount {
    pub fn count(&self) -> u8 {
        match self {
            DigitCount::Unspecified | DigitCount::Six => 6,
            DigitCount::Eight => 8,
        }
    }
}
