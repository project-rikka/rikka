use rikka_types::Choice;
use serde::{Deserialize, Serialize};

use crate::preferences::list::Pref;

type Prefs = Vec<(&'static str, Pref)>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CookiePolicy {
    AcceptAll,
    RejectThirdParty,
    RejectAll,
    AcceptVisited,
    RejectTrackers,
    #[default]
    PartitionThirdParty,
}

impl Choice for CookiePolicy {
    const ALL: &'static [CookiePolicy] = &[
        CookiePolicy::PartitionThirdParty,
        CookiePolicy::RejectTrackers,
        CookiePolicy::RejectThirdParty,
        CookiePolicy::AcceptVisited,
        CookiePolicy::RejectAll,
        CookiePolicy::AcceptAll,
    ];

    fn slug(self) -> &'static str {
        match self {
            CookiePolicy::AcceptAll => "accept-all",
            CookiePolicy::RejectThirdParty => "reject-third-party",
            CookiePolicy::RejectAll => "reject-all",
            CookiePolicy::AcceptVisited => "accept-visited",
            CookiePolicy::RejectTrackers => "reject-trackers",
            CookiePolicy::PartitionThirdParty => "partition-third-party",
        }
    }
}

impl CookiePolicy {
    fn code(self) -> i32 {
        match self {
            CookiePolicy::AcceptAll => 0,
            CookiePolicy::RejectThirdParty => 1,
            CookiePolicy::RejectAll => 2,
            CookiePolicy::AcceptVisited => 3,
            CookiePolicy::RejectTrackers => 4,
            CookiePolicy::PartitionThirdParty => 5,
        }
    }

    pub(crate) fn prefs(self) -> Prefs {
        vec![
            ("network.cookie.cookieBehavior", self.code().into()),
            ("network.cookie.cookieBehavior.pbmode", self.code().into()),
        ]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrackingProtection {
    Off,
    #[default]
    Standard,
    Strict,
}

impl Choice for TrackingProtection {
    const ALL: &'static [TrackingProtection] = &[
        TrackingProtection::Standard,
        TrackingProtection::Strict,
        TrackingProtection::Off,
    ];

    fn slug(self) -> &'static str {
        match self {
            TrackingProtection::Off => "off",
            TrackingProtection::Standard => "standard",
            TrackingProtection::Strict => "strict",
        }
    }
}

impl TrackingProtection {
    pub(crate) fn prefs(self) -> Prefs {
        let (standard, strict) = match self {
            TrackingProtection::Off => (false, false),
            TrackingProtection::Standard => (true, false),
            TrackingProtection::Strict => (true, true),
        };
        let category = match self {
            TrackingProtection::Off => "custom",
            TrackingProtection::Standard => "standard",
            TrackingProtection::Strict => "strict",
        };
        vec![
            ("browser.contentblocking.category", category.into()),
            (
                "privacy.trackingprotection.annotate_channels",
                standard.into(),
            ),
            (
                "privacy.trackingprotection.cryptomining.enabled",
                standard.into(),
            ),
            (
                "privacy.trackingprotection.fingerprinting.enabled",
                standard.into(),
            ),
            (
                "privacy.socialtracking.block_cookies.enabled",
                standard.into(),
            ),
            (
                "privacy.trackingprotection.emailtracking.pbmode.enabled",
                standard.into(),
            ),
            ("privacy.query_stripping.enabled.pbmode", standard.into()),
            (
                "privacy.trackingprotection.socialtracking.enabled",
                strict.into(),
            ),
            (
                "privacy.annotate_channels.strict_list.enabled",
                strict.into(),
            ),
            (
                "privacy.trackingprotection.emailtracking.enabled",
                strict.into(),
            ),
            ("privacy.query_stripping.enabled", strict.into()),
        ]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum HttpsOnly {
    #[default]
    Off,
    PrivateOnly,
    Everywhere,
}

impl HttpsOnly {
    pub(crate) fn prefs(self) -> Prefs {
        vec![
            (
                "dom.security.https_only_mode",
                (self == HttpsOnly::Everywhere).into(),
            ),
            (
                "dom.security.https_only_mode_pbm",
                (self != HttpsOnly::Off).into(),
            ),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
}

impl Endpoint {
    pub fn new(host: impl Into<String>, port: u16) -> Endpoint {
        Endpoint {
            host: host.into(),
            port,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ManualProxy {
    pub http: Option<Endpoint>,
    pub https: Option<Endpoint>,
    pub socks: Option<Endpoint>,
    pub socks_resolves_names: bool,
    pub bypass: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Proxy {
    Direct,
    #[default]
    System,
    Detect,
    Script(String),
    Manual(ManualProxy),
}

impl Proxy {
    pub(crate) fn prefs(self) -> Prefs {
        match self {
            Proxy::Direct => vec![("network.proxy.type", 0.into())],
            Proxy::System => vec![("network.proxy.type", 5.into())],
            Proxy::Detect => vec![("network.proxy.type", 4.into())],
            Proxy::Script(url) => vec![
                ("network.proxy.type", 2.into()),
                ("network.proxy.autoconfig_url", url.into()),
            ],
            Proxy::Manual(manual) => manual_prefs(manual),
        }
    }
}

fn manual_prefs(manual: ManualProxy) -> Prefs {
    let mut prefs = vec![
        ("network.proxy.type", 1.into()),
        ("network.proxy.share_proxy_settings", false.into()),
        (
            "network.proxy.socks_remote_dns",
            manual.socks_resolves_names.into(),
        ),
        (
            "network.proxy.no_proxies_on",
            manual.bypass.join(", ").into(),
        ),
    ];
    let endpoints = [
        ("network.proxy.http", "network.proxy.http_port", manual.http),
        ("network.proxy.ssl", "network.proxy.ssl_port", manual.https),
        (
            "network.proxy.socks",
            "network.proxy.socks_port",
            manual.socks,
        ),
    ];
    for (host, port, endpoint) in endpoints {
        let endpoint = endpoint.unwrap_or(Endpoint::new("", 0));
        prefs.push((host, endpoint.host.into()));
        prefs.push((port, i32::from(endpoint.port).into()));
    }
    prefs
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DnsOverHttps {
    #[default]
    Off,
    Preferred(String),
    Only(String),
}

impl DnsOverHttps {
    pub(crate) fn prefs(self) -> Prefs {
        match self {
            DnsOverHttps::Off => vec![("network.trr.mode", 5.into())],
            DnsOverHttps::Preferred(uri) => vec![
                ("network.trr.mode", 2.into()),
                ("network.trr.uri", uri.into()),
            ],
            DnsOverHttps::Only(uri) => vec![
                ("network.trr.mode", 3.into()),
                ("network.trr.uri", uri.into()),
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Spellcheck {
    Off,
    #[default]
    MultiLine,
    Everywhere,
}

impl Choice for Spellcheck {
    const ALL: &'static [Spellcheck] = &[
        Spellcheck::MultiLine,
        Spellcheck::Everywhere,
        Spellcheck::Off,
    ];

    fn slug(self) -> &'static str {
        match self {
            Spellcheck::Off => "off",
            Spellcheck::MultiLine => "multi-line",
            Spellcheck::Everywhere => "everywhere",
        }
    }
}

impl Spellcheck {
    pub(crate) fn prefs(self) -> Prefs {
        let code = match self {
            Spellcheck::Off => 0,
            Spellcheck::MultiLine => 1,
            Spellcheck::Everywhere => 2,
        };
        vec![("layout.spellcheckDefault", code.into())]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Autoplay {
    Allowed,
    #[default]
    BlockAudible,
    BlockAll,
}

impl Choice for Autoplay {
    const ALL: &'static [Autoplay] = &[
        Autoplay::BlockAudible,
        Autoplay::BlockAll,
        Autoplay::Allowed,
    ];

    fn slug(self) -> &'static str {
        match self {
            Autoplay::Allowed => "allowed",
            Autoplay::BlockAudible => "block-audible",
            Autoplay::BlockAll => "block-all",
        }
    }
}

impl Autoplay {
    pub(crate) fn prefs(self) -> Prefs {
        let code = match self {
            Autoplay::Allowed => 0,
            Autoplay::BlockAudible => 1,
            Autoplay::BlockAll => 5,
        };
        vec![("media.autoplay.default", code.into())]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ColorScheme {
    Light,
    Dark,
    #[default]
    System,
}

impl Choice for ColorScheme {
    const ALL: &'static [ColorScheme] =
        &[ColorScheme::System, ColorScheme::Light, ColorScheme::Dark];

    fn slug(self) -> &'static str {
        match self {
            ColorScheme::Light => "light",
            ColorScheme::Dark => "dark",
            ColorScheme::System => "system",
        }
    }
}

impl ColorScheme {
    pub(crate) fn prefs(self) -> Prefs {
        let code = match self {
            ColorScheme::Dark => 0,
            ColorScheme::Light => 1,
            ColorScheme::System => 2,
        };
        vec![(
            "layout.css.prefers-color-scheme.content-override",
            code.into(),
        )]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SitePermission {
    Location,
    Camera,
    Microphone,
    Notifications,
    ProtectedMedia,
}

impl SitePermission {
    pub const ALL: [SitePermission; 5] = [
        SitePermission::Location,
        SitePermission::Camera,
        SitePermission::Microphone,
        SitePermission::Notifications,
        SitePermission::ProtectedMedia,
    ];

    fn default_pref(self) -> &'static str {
        match self {
            SitePermission::Location => "permissions.default.geo",
            SitePermission::Camera => "permissions.default.camera",
            SitePermission::Microphone => "permissions.default.microphone",
            SitePermission::Notifications => "permissions.default.desktop-notification",
            SitePermission::ProtectedMedia => "permissions.default.media-key-system-access",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionDefault {
    #[default]
    Ask,
    Block,
}

impl Choice for PermissionDefault {
    const ALL: &'static [PermissionDefault] = &[PermissionDefault::Ask, PermissionDefault::Block];

    fn slug(self) -> &'static str {
        match self {
            PermissionDefault::Ask => "ask",
            PermissionDefault::Block => "block",
        }
    }
}

impl PermissionDefault {
    pub(crate) fn prefs(self, permission: SitePermission) -> Prefs {
        let code = match self {
            PermissionDefault::Ask => 0,
            PermissionDefault::Block => 2,
        };
        vec![(permission.default_pref(), code.into())]
    }
}
