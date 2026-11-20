use rikka_web::{
    Autoplay, ColorScheme, CookiePolicy, DnsOverHttps, Locales, Preferences, Proxy,
    SessionSettings, Spellcheck, TrackingProtection, UserContent, UserStyle,
};
use serde::{Deserialize, Serialize};

use crate::permissions::{Permission, PermissionDefaults};

pub const USER_STYLE: &str = "rikka-user-style";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineSettings {
    pub tracking_protection: TrackingProtection,
    pub cookies: CookiePolicy,
    pub dns_over_https: DnsOverHttps,
    pub proxy: Proxy,
    pub page_languages: Vec<String>,
    pub autoplay: Autoplay,
    pub color_scheme: ColorScheme,
    pub spellcheck: Spellcheck,
    pub dictionaries: Vec<String>,
    pub user_style: String,
    pub permissions: PermissionDefaults,
}

impl EngineSettings {
    pub fn preferences(&self) -> Preferences {
        let preferences = Preferences::new()
            .tracking_protection(self.tracking_protection)
            .cookies(self.cookies)
            .dns_over_https(self.dns_over_https.clone())
            .proxy(self.proxy.clone())
            .autoplay(self.autoplay)
            .color_scheme(self.color_scheme)
            .spellcheck(self.spellcheck)
            .dictionaries(self.dictionaries.iter().cloned());
        Permission::ALL
            .into_iter()
            .filter_map(|permission| Some((permission.engine()?, self.permissions.get(permission))))
            .fold(preferences, |preferences, (permission, default)| {
                preferences.permission_default(permission, default)
            })
    }

    pub fn locales(&self, interface: &str) -> Locales {
        let locales = Locales::new([interface]);
        match self.page_languages.is_empty() {
            true => locales,
            false => locales.pages(self.page_languages.iter().cloned()),
        }
    }

    pub fn session(&self, settings: SessionSettings) -> SessionSettings {
        settings.tracking_protection(self.tracking_protection != TrackingProtection::Off)
    }

    pub fn dress(&self, content: UserContent) -> UserContent {
        let source = self.user_style.trim();
        match source.is_empty() {
            true => content.without(USER_STYLE),
            false => content.style(UserStyle::new(USER_STYLE, source)),
        }
    }
}
