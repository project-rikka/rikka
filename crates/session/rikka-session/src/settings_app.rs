use std::ffi::OsStr;

pub const PROGRAM: &str = "rikka-settings";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Section {
    Users,
    #[default]
    Appearance,
    Windows,
    Notifications,
    Lock,
    Keyring,
    Capture,
    Replay,
    ColorPicker,
    Displays,
    Pointer,
    Keyboard,
    Audio,
    Power,
    Network,
    Bluetooth,
    Remote,
    Language,
    DateTime,
    Defaults,
    Llm,
    System,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Spot {
    Wallpaper,
    SoundOutput,
    SoundInput,
    InputSources,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Link {
    Section(Section),
    Spot(Spot),
}

impl Section {
    pub const ALL: [Section; 22] = [
        Section::Users,
        Section::Appearance,
        Section::Windows,
        Section::Notifications,
        Section::Lock,
        Section::Keyring,
        Section::Capture,
        Section::Replay,
        Section::ColorPicker,
        Section::Displays,
        Section::Pointer,
        Section::Keyboard,
        Section::Audio,
        Section::Power,
        Section::Network,
        Section::Bluetooth,
        Section::Remote,
        Section::Language,
        Section::DateTime,
        Section::Defaults,
        Section::Llm,
        Section::System,
    ];

    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Section::Users => "users",
            Section::Appearance => "appearance",
            Section::Windows => "windows",
            Section::Notifications => "notifications",
            Section::Lock => "lock-screen",
            Section::Keyring => "keyring",
            Section::Capture => "capture",
            Section::Replay => "replay",
            Section::ColorPicker => "color-picker",
            Section::Displays => "displays",
            Section::Pointer => "pointer",
            Section::Keyboard => "keyboard",
            Section::Audio => "audio",
            Section::Power => "power",
            Section::Network => "network",
            Section::Bluetooth => "bluetooth",
            Section::Remote => "remote",
            Section::Language => "language",
            Section::DateTime => "date-time",
            Section::Defaults => "default-apps",
            Section::Llm => "llm",
            Section::System => "system",
        }
    }

    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Section> {
        Section::ALL
            .into_iter()
            .find(|section| section.slug() == slug)
    }
}

impl Spot {
    pub const ALL: [Spot; 4] = [
        Spot::Wallpaper,
        Spot::SoundOutput,
        Spot::SoundInput,
        Spot::InputSources,
    ];

    #[must_use]
    pub const fn section(self) -> Section {
        match self {
            Spot::Wallpaper => Section::Appearance,
            Spot::SoundOutput | Spot::SoundInput => Section::Audio,
            Spot::InputSources => Section::Keyboard,
        }
    }

    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Spot::Wallpaper => "wallpaper",
            Spot::SoundOutput => "output",
            Spot::SoundInput => "input",
            Spot::InputSources => "sources",
        }
    }

    #[must_use]
    pub fn from_slug(section: Section, slug: &str) -> Option<Spot> {
        Spot::ALL
            .into_iter()
            .find(|spot| spot.section() == section && spot.slug() == slug)
    }
}

impl Link {
    #[must_use]
    pub const fn section(self) -> Section {
        match self {
            Link::Section(section) => section,
            Link::Spot(spot) => spot.section(),
        }
    }

    #[must_use]
    pub const fn spot(self) -> Option<Spot> {
        match self {
            Link::Section(_) => None,
            Link::Spot(spot) => Some(spot),
        }
    }

    #[must_use]
    pub fn argv(self) -> Vec<&'static str> {
        let mut argv = vec![PROGRAM, self.section().slug()];
        argv.extend(self.spot().map(Spot::slug));
        argv
    }

    #[must_use]
    pub fn from_arguments<A: AsRef<OsStr>>(arguments: impl IntoIterator<Item = A>) -> Option<Link> {
        let mut words = arguments.into_iter();
        let section = words
            .by_ref()
            .find_map(|word| word.as_ref().to_str().and_then(Section::from_slug))?;
        let spot = words
            .next()
            .and_then(|word| Spot::from_slug(section, word.as_ref().to_str()?));
        Some(spot.map_or(Link::Section(section), Link::Spot))
    }
}

impl From<Section> for Link {
    fn from(section: Section) -> Link {
        Link::Section(section)
    }
}

impl From<Spot> for Link {
    fn from(spot: Spot) -> Link {
        Link::Spot(spot)
    }
}
