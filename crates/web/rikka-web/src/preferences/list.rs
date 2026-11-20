use std::collections::BTreeMap;

use rikka_gecko_protocol::geckoview::preferences::{PrefValue, SetDefaultPrefs};

use crate::preferences::choices::{
    Autoplay, ColorScheme, CookiePolicy, DnsOverHttps, HttpsOnly, PermissionDefault, Proxy,
    SitePermission, Spellcheck, TrackingProtection,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pref {
    Bool(bool),
    Int(i32),
    Text(String),
}

impl From<bool> for Pref {
    fn from(value: bool) -> Pref {
        Pref::Bool(value)
    }
}

impl From<i32> for Pref {
    fn from(value: i32) -> Pref {
        Pref::Int(value)
    }
}

impl From<&str> for Pref {
    fn from(value: &str) -> Pref {
        Pref::Text(value.to_owned())
    }
}

impl From<String> for Pref {
    fn from(value: String) -> Pref {
        Pref::Text(value)
    }
}

impl From<Pref> for PrefValue {
    fn from(pref: Pref) -> PrefValue {
        match pref {
            Pref::Bool(value) => PrefValue::Bool(value),
            Pref::Int(value) => PrefValue::Int(value),
            Pref::Text(value) => PrefValue::String(value),
        }
    }
}

#[must_use]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preferences {
    values: BTreeMap<String, Pref>,
}

impl Preferences {
    pub fn new() -> Preferences {
        Preferences::default()
    }

    pub fn set(mut self, name: impl Into<String>, value: impl Into<Pref>) -> Preferences {
        self.values.insert(name.into(), value.into());
        self
    }

    pub fn cookies(self, policy: CookiePolicy) -> Preferences {
        self.all(policy.prefs())
    }

    pub fn tracking_protection(self, level: TrackingProtection) -> Preferences {
        self.all(level.prefs())
    }

    pub fn https_only(self, mode: HttpsOnly) -> Preferences {
        self.all(mode.prefs())
    }

    pub fn proxy(self, proxy: Proxy) -> Preferences {
        self.all(proxy.prefs())
    }

    pub fn dns_over_https(self, mode: DnsOverHttps) -> Preferences {
        self.all(mode.prefs())
    }

    pub fn spellcheck(self, spellcheck: Spellcheck) -> Preferences {
        self.all(spellcheck.prefs())
    }

    pub fn dictionaries<S: Into<String>>(
        self,
        dictionaries: impl IntoIterator<Item = S>,
    ) -> Preferences {
        let joined = dictionaries
            .into_iter()
            .map(Into::into)
            .collect::<Vec<String>>()
            .join(",");
        self.set("spellchecker.dictionary", joined)
    }

    pub fn autoplay(self, autoplay: Autoplay) -> Preferences {
        self.all(autoplay.prefs())
    }

    pub fn color_scheme(self, scheme: ColorScheme) -> Preferences {
        self.all(scheme.prefs())
    }

    pub fn permission_default(
        self,
        permission: SitePermission,
        default: PermissionDefault,
    ) -> Preferences {
        self.all(default.prefs(permission))
    }

    pub(crate) fn defaults(&self) -> SetDefaultPrefs {
        SetDefaultPrefs(
            self.values
                .iter()
                .map(|(name, value)| (name.clone(), value.clone().into()))
                .collect(),
        )
    }

    pub(crate) fn all(mut self, prefs: Vec<(&'static str, Pref)>) -> Preferences {
        for (name, value) in prefs {
            self.values.insert(name.to_owned(), value);
        }
        self
    }
}
