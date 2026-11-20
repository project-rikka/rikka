mod choices;
mod list;
mod locales;
mod scrolling;

pub use choices::{
    Autoplay, ColorScheme, CookiePolicy, DnsOverHttps, Endpoint, HttpsOnly, ManualProxy,
    PermissionDefault, Proxy, SitePermission, Spellcheck, TrackingProtection,
};
pub use list::{Pref, Preferences};
pub use locales::Locales;
