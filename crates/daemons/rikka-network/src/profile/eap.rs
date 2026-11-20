use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EapMethod {
    #[default]
    Peap,
    Ttls,
    Tls,
    Pwd,
}

impl EapMethod {
    pub(crate) fn read(name: &str) -> Self {
        match name {
            "ttls" => Self::Ttls,
            "tls" => Self::Tls,
            "pwd" => Self::Pwd,
            _ => Self::Peap,
        }
    }

    pub(crate) fn write(self) -> &'static str {
        match self {
            Self::Peap => "peap",
            Self::Ttls => "ttls",
            Self::Tls => "tls",
            Self::Pwd => "pwd",
        }
    }

    #[must_use]
    pub fn is_tunnelled(self) -> bool {
        matches!(self, Self::Peap | Self::Ttls)
    }

    #[must_use]
    pub fn is_certificate(self) -> bool {
        self == Self::Tls
    }

    #[must_use]
    pub fn inner(self) -> &'static [Phase2] {
        match self {
            Self::Peap => &[Phase2::Mschapv2, Phase2::Md5, Phase2::Gtc],
            Self::Ttls => &[Phase2::Mschapv2, Phase2::Pap, Phase2::Mschap, Phase2::Chap],
            Self::Tls | Self::Pwd => &[],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase2 {
    #[default]
    Mschapv2,
    Pap,
    Chap,
    Mschap,
    Md5,
    Gtc,
}

impl Phase2 {
    pub(crate) fn read(name: &str) -> Self {
        match name {
            "pap" => Self::Pap,
            "chap" => Self::Chap,
            "mschap" => Self::Mschap,
            "md5" => Self::Md5,
            "gtc" => Self::Gtc,
            _ => Self::Mschapv2,
        }
    }

    pub(crate) fn write(self) -> &'static str {
        match self {
            Self::Mschapv2 => "mschapv2",
            Self::Pap => "pap",
            Self::Chap => "chap",
            Self::Mschap => "mschap",
            Self::Md5 => "md5",
            Self::Gtc => "gtc",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Eap {
    pub method: EapMethod,
    pub identity: String,
    pub anonymous_identity: String,
    pub authority: Option<PathBuf>,
    pub domain_suffix: String,
    pub certificate: Option<PathBuf>,
    pub private_key: Option<PathBuf>,
    pub private_key_password: Option<String>,
    pub phase2: Phase2,
    pub password: Option<String>,
}

impl Eap {
    #[must_use]
    pub fn is_complete(&self) -> bool {
        match self.method {
            EapMethod::Tls => self.certificate.is_some() && self.private_key.is_some(),
            _ => !self.identity.is_empty(),
        }
    }
}
