use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};

const SITE_DETAILS: &str = "chrome://settings/content/siteDetails?site=";

#[must_use]
pub fn site_details(origin: &str) -> String {
    format!(
        "{SITE_DETAILS}{}",
        utf8_percent_encode(origin, NON_ALPHANUMERIC)
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChromePage {
    Settings,
    Account,
    Sync,
    Passwords,
    Autofill,
    Payments,
    Addresses,
    Privacy,
    Security,
    SiteSettings,
    SearchEngines,
    Downloads,
    Languages,
    History,
    Bookmarks,
    Extensions,
}

impl ChromePage {
    pub const ALL: [ChromePage; 16] = [
        ChromePage::Settings,
        ChromePage::Account,
        ChromePage::Sync,
        ChromePage::Passwords,
        ChromePage::Autofill,
        ChromePage::Payments,
        ChromePage::Addresses,
        ChromePage::Privacy,
        ChromePage::Security,
        ChromePage::SiteSettings,
        ChromePage::SearchEngines,
        ChromePage::Downloads,
        ChromePage::Languages,
        ChromePage::History,
        ChromePage::Bookmarks,
        ChromePage::Extensions,
    ];

    #[must_use]
    pub fn address(self) -> &'static str {
        match self {
            ChromePage::Settings => "chrome://settings",
            ChromePage::Account => "chrome://settings/people",
            ChromePage::Sync => "chrome://settings/syncSetup",
            ChromePage::Passwords => "chrome://password-manager/passwords",
            ChromePage::Autofill => "chrome://settings/autofill",
            ChromePage::Payments => "chrome://settings/payments",
            ChromePage::Addresses => "chrome://settings/addresses",
            ChromePage::Privacy => "chrome://settings/privacy",
            ChromePage::Security => "chrome://settings/security",
            ChromePage::SiteSettings => "chrome://settings/content",
            ChromePage::SearchEngines => "chrome://settings/searchEngines",
            ChromePage::Downloads => "chrome://settings/downloads",
            ChromePage::Languages => "chrome://settings/languages",
            ChromePage::History => "chrome://history",
            ChromePage::Bookmarks => "chrome://bookmarks",
            ChromePage::Extensions => "chrome://extensions",
        }
    }
}
