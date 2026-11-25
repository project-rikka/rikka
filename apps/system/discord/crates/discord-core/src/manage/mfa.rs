#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MfaMethod {
    Totp,
    Sms,
    Backup,
    Password,
}

impl MfaMethod {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            MfaMethod::Totp => "totp",
            MfaMethod::Sms => "sms",
            MfaMethod::Backup => "backup",
            MfaMethod::Password => "password",
        }
    }

    #[must_use]
    pub fn named(name: &str) -> Option<MfaMethod> {
        match name {
            "totp" => Some(MfaMethod::Totp),
            "sms" => Some(MfaMethod::Sms),
            "backup" => Some(MfaMethod::Backup),
            "password" => Some(MfaMethod::Password),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MfaChallenge {
    pub ticket: String,
    pub methods: Vec<MfaMethod>,
}
