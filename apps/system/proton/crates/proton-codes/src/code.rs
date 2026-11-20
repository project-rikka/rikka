use rikka_proton_authenticator::Otp;

const PERIOD: u16 = 30;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Code {
    pub now: String,
    pub remaining: u16,
    pub period: u16,
}

impl Code {
    #[must_use]
    pub fn at(otp: &Otp, at: jiff::Timestamp) -> Option<Code> {
        let codes = otp.codes_at(at).ok()?;

        Some(Code {
            now: codes.current,
            remaining: otp.remaining_at(at).unwrap_or(0),
            period: otp.period().unwrap_or(PERIOD),
        })
    }

    #[must_use]
    pub fn grouped(&self) -> String {
        match self.now.len() == 6 {
            true => format!("{} {}", &self.now[..3], &self.now[3..]),
            false => self.now.clone(),
        }
    }

    #[must_use]
    pub fn remaining_fraction(&self) -> f32 {
        f32::from(self.remaining) / f32::from(self.period.max(1))
    }
}

#[must_use]
pub fn code_at(uri: &str, at: jiff::Timestamp) -> Option<Code> {
    Code::at(&Otp::parse(&otpauth_uri(uri)).ok()?, at)
}

fn otpauth_uri(uri: &str) -> String {
    let trimmed = uri.trim();
    if trimmed.contains("://") {
        return trimmed.to_owned();
    }

    let secret: String = trimmed.chars().filter(|letter| *letter != ' ').collect();
    format!("otpauth://totp/code?secret={secret}")
}
