use rikka_proton_authenticator::{Algorithm, Hotp, Otp, Steam, Totp};

use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Totp,
    Steam,
}

impl Kind {
    pub const ALL: [Kind; 2] = [Kind::Totp, Kind::Steam];
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Setup {
    pub kind: Kind,
    pub title: String,
    pub secret: String,
    pub issuer: String,
    pub digits: u8,
    pub period: u16,
    pub algorithm: Algorithm,
    pub counter: Option<u64>,
}

impl Default for Setup {
    fn default() -> Setup {
        Setup {
            kind: Kind::Totp,
            title: String::new(),
            secret: String::new(),
            issuer: String::new(),
            digits: Totp::DIGITS,
            period: Totp::PERIOD,
            algorithm: Algorithm::Sha1,
            counter: None,
        }
    }
}

impl Setup {
    pub const DIGITS: [u8; 3] = [6, 7, 8];
    pub const PERIODS: [u16; 2] = [30, 60];
    pub const ALGORITHMS: [Algorithm; 3] = [Algorithm::Sha1, Algorithm::Sha256, Algorithm::Sha512];

    #[must_use]
    pub fn from_otp(otp: &Otp) -> Setup {
        let named = Setup {
            title: otp.label().unwrap_or_default().to_owned(),
            secret: otp.secret().to_owned(),
            ..Setup::default()
        };
        match otp {
            Otp::Totp(totp) => Setup {
                issuer: totp.issuer.clone().unwrap_or_default(),
                digits: totp.digits,
                period: totp.period,
                algorithm: totp.algorithm,
                ..named
            },
            Otp::Hotp(hotp) => Setup {
                issuer: hotp.issuer.clone().unwrap_or_default(),
                digits: hotp.digits,
                algorithm: hotp.algorithm,
                counter: Some(hotp.counter),
                ..named
            },
            Otp::Steam(_) => Setup {
                kind: Kind::Steam,
                ..named
            },
        }
    }

    #[must_use]
    pub fn read(link: &str) -> Option<Setup> {
        let trimmed = link.trim();
        if !trimmed.contains("://") {
            return None;
        }
        Otp::parse(trimmed).ok().map(|otp| Setup::from_otp(&otp))
    }

    #[must_use]
    pub fn digits_offered(&self) -> Vec<u8> {
        offered(&Setup::DIGITS, self.digits)
    }

    #[must_use]
    pub fn periods_offered(&self) -> Vec<u16> {
        offered(&Setup::PERIODS, self.period)
    }

    pub fn otp(&self) -> Result<Otp> {
        let title = self.title.trim();
        if title.is_empty() {
            return Err(Error::Untitled);
        }
        let secret: String = self
            .secret
            .chars()
            .filter(|letter| !letter.is_whitespace())
            .collect();
        if secret.is_empty() {
            return Err(Error::NoSecret);
        }
        let issuer = self.issuer.trim();
        let issuer = (!issuer.is_empty()).then(|| issuer.to_owned());

        let otp = match (self.kind, self.counter) {
            (Kind::Steam, _) => Otp::Steam(Steam {
                secret,
                name: Some(title.to_owned()),
            }),
            (Kind::Totp, Some(counter)) => Otp::Hotp(Hotp {
                label: Some(title.to_owned()),
                issuer,
                secret: secret.to_uppercase(),
                algorithm: self.algorithm,
                digits: self.digits,
                counter,
            }),
            (Kind::Totp, None) => Otp::Totp(Totp {
                label: Some(title.to_owned()),
                issuer,
                secret: secret.to_uppercase(),
                algorithm: self.algorithm,
                digits: self.digits,
                period: self.period,
            }),
        };
        otp.secret_bytes()?;
        Ok(otp)
    }
}

fn offered<T: Copy + Ord>(usual: &[T], current: T) -> Vec<T> {
    let mut offered = usual.to_vec();
    if !offered.contains(&current) {
        offered.push(current);
        offered.sort_unstable();
    }
    offered
}
