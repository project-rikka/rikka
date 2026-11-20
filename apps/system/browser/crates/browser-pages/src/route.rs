use url::form_urlencoded;

pub const SCHEME: &str = "rikka";
pub const VIEW_SOURCE: &str = "view-source:";
const SOURCE_QUERY: &str = "source?";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Internal {
    Home,
    Settings(Option<Section>),
    History,
    Archive,
    Downloads,
    Extensions,
    Source(String),
    Missing(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    Appearance,
    Tabs,
    Home,
    Profiles,
    Search,
    DefaultBrowser,
    Translation,
    Privacy,
    Permissions,
    Sites,
    Content,
    Network,
    Extensions,
    HttpsOnly,
    Downloads,
    Zoom,
    Performance,
    ClearData,
}

impl Section {
    pub const LISTED: [Section; 17] = [
        Section::Appearance,
        Section::Tabs,
        Section::Home,
        Section::Profiles,
        Section::Search,
        Section::DefaultBrowser,
        Section::Translation,
        Section::Privacy,
        Section::Permissions,
        Section::Sites,
        Section::Content,
        Section::Network,
        Section::Extensions,
        Section::HttpsOnly,
        Section::Downloads,
        Section::Zoom,
        Section::Performance,
    ];

    const ALL: [Section; 18] = [
        Section::Appearance,
        Section::Tabs,
        Section::Home,
        Section::Profiles,
        Section::Search,
        Section::DefaultBrowser,
        Section::Translation,
        Section::Privacy,
        Section::Permissions,
        Section::Sites,
        Section::Content,
        Section::Network,
        Section::Extensions,
        Section::HttpsOnly,
        Section::Downloads,
        Section::Zoom,
        Section::Performance,
        Section::ClearData,
    ];

    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Section::Appearance => "appearance",
            Section::Tabs => "tabs",
            Section::Home => "home",
            Section::Profiles => "profiles",
            Section::Search => "search",
            Section::DefaultBrowser => "default-browser",
            Section::Translation => "translation",
            Section::Privacy => "privacy",
            Section::Permissions => "permissions",
            Section::Content => "content",
            Section::Network => "network",
            Section::Sites => "sites",
            Section::Extensions => "extensions",
            Section::HttpsOnly => "https-only",
            Section::Downloads => "downloads",
            Section::Zoom => "zoom",
            Section::Performance => "performance",
            Section::ClearData => "clear-browsing-data",
        }
    }

    #[must_use]
    pub fn listed_under(self) -> Section {
        match self {
            Section::ClearData => Section::Privacy,
            listed => listed,
        }
    }
}

impl Internal {
    #[must_use]
    pub fn parse(address: &str) -> Option<Internal> {
        if let Some(page) = address.strip_prefix(VIEW_SOURCE) {
            return Some(Internal::Source(page.to_owned()));
        }
        let rest = address.strip_prefix(SCHEME)?.strip_prefix("://")?;
        if let Some(page) = source_of(rest) {
            return Some(Internal::Source(page));
        }
        let rest = rest.trim_end_matches('/');
        let (page, section) = rest.split_once('/').unwrap_or((rest, ""));
        let known = match (page, section) {
            ("home", "") => Some(Internal::Home),
            ("settings", "") => Some(Internal::Settings(None)),
            ("settings", slug) => Section::ALL
                .into_iter()
                .find(|section| section.slug() == slug)
                .map(|section| Internal::Settings(Some(section))),
            ("history", "") => Some(Internal::History),
            ("archive", "") => Some(Internal::Archive),
            ("downloads", "") => Some(Internal::Downloads),
            ("extensions", "") => Some(Internal::Extensions),
            _ => None,
        };
        Some(known.unwrap_or_else(|| Internal::Missing(address.to_owned())))
    }

    #[must_use]
    pub fn address(&self) -> String {
        match self {
            Internal::Home => format!("{SCHEME}://home"),
            Internal::Settings(None) => format!("{SCHEME}://settings"),
            Internal::Settings(Some(section)) => {
                format!("{SCHEME}://settings/{}", section.slug())
            }
            Internal::History => format!("{SCHEME}://history"),
            Internal::Archive => format!("{SCHEME}://archive"),
            Internal::Downloads => format!("{SCHEME}://downloads"),
            Internal::Extensions => format!("{SCHEME}://extensions"),
            Internal::Source(page) => source_address(page),
            Internal::Missing(address) => address.clone(),
        }
    }
}

fn source_of(rest: &str) -> Option<String> {
    let query = rest.strip_prefix(SOURCE_QUERY)?;
    form_urlencoded::parse(query.as_bytes())
        .find(|(key, _)| key == "url")
        .map(|(_, page)| page.into_owned())
}

fn source_address(page: &str) -> String {
    let encoded: String = form_urlencoded::byte_serialize(page.as_bytes()).collect();
    format!("{SCHEME}://{SOURCE_QUERY}url={encoded}")
}
