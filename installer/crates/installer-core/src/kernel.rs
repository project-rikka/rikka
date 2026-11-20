#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kernel {
    Default,
    Latest,
    #[default]
    Zen,
}

impl Kernel {
    pub const ALL: &'static [Kernel] = &[Kernel::Default, Kernel::Latest, Kernel::Zen];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Default => "default (LTS)",
            Self::Latest => "latest stable",
            Self::Zen => "zen",
        }
    }

    #[must_use]
    pub fn packages(self) -> Option<&'static str> {
        match self {
            Self::Default => None,
            Self::Latest => Some("linuxPackages_latest"),
            Self::Zen => Some("linuxPackages_zen"),
        }
    }
}
