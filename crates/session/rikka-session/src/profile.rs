use std::ffi::OsStr;

pub const PROFILE_ENV: &str = "RIKKA_SESSION_PROFILE";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Profile {
    #[default]
    Full,
    Simple,
}

const PROFILES: [(Profile, &str); 2] = [(Profile::Full, "full"), (Profile::Simple, "simple")];

impl Profile {
    #[must_use]
    pub fn from_environment() -> Profile {
        std::env::var_os(PROFILE_ENV)
            .as_deref()
            .and_then(OsStr::to_str)
            .and_then(Profile::from_name)
            .unwrap_or_default()
    }

    #[must_use]
    fn from_name(name: &str) -> Option<Profile> {
        PROFILES
            .iter()
            .find(|(_, stored)| *stored == name)
            .map(|(profile, _)| *profile)
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        PROFILES
            .iter()
            .find(|(profile, _)| *profile == self)
            .map(|(_, stored)| *stored)
            .unwrap_or_default()
    }

    #[must_use]
    pub fn is_full(self) -> bool {
        self == Profile::Full
    }

    #[must_use]
    pub fn is_simple(self) -> bool {
        self == Profile::Simple
    }
}
