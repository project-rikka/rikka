pub const SCHEME: &str = "rikka";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Internal {
    Home,
    Settings(Option<Section>),
    History,
    Archive,
    Downloads,
    Extensions,
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
    Extensions,
    Performance,
    Chrome,
    ClearData,
}

impl Section {
    pub const LISTED: [Section; 11] = [
        Section::Appearance,
        Section::Tabs,
        Section::Home,
        Section::Profiles,
        Section::Search,
        Section::DefaultBrowser,
        Section::Translation,
        Section::Privacy,
        Section::Extensions,
        Section::Performance,
        Section::Chrome,
    ];

    const ALL: [Section; 12] = [
        Section::Appearance,
        Section::Tabs,
        Section::Home,
        Section::Profiles,
        Section::Search,
        Section::DefaultBrowser,
        Section::Translation,
        Section::Privacy,
        Section::Extensions,
        Section::Performance,
        Section::Chrome,
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
            Section::Extensions => "extensions",
            Section::Performance => "performance",
            Section::Chrome => "chrome",
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
        let rest = address.strip_prefix(SCHEME)?.strip_prefix("://")?;
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
            Internal::Missing(address) => address.clone(),
        }
    }
}
