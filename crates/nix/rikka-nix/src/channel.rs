use rikka_nix_edit::Document;
use rikka_nixos_layout::Location;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Channel {
    Unstable,
    Release(String),
}

const VERSION: &str = "/run/current-system/nixos-version";

const RIKKA_NIXPKGS: &str = "rikka/nixpkgs";

#[must_use]
pub fn running_version() -> Option<String> {
    std::fs::read_to_string(VERSION).ok()
}

impl Channel {
    #[must_use]
    pub fn detect() -> Self {
        Location::system()
            .ok()
            .and_then(|location| std::fs::read_to_string(location.flake_file()).ok())
            .and_then(|flake| Self::from_flake(&flake))
            .or_else(|| running_version().and_then(|version| Self::from_version(&version)))
            .unwrap_or(Self::Unstable)
    }

    #[must_use]
    pub fn from_flake(source: &str) -> Option<Self> {
        let flake = Document::parse(source).ok()?;
        if let Ok(Some(url)) = flake.get::<String>("inputs.nixpkgs.url") {
            return Self::from_url(&url);
        }
        let followed: String = flake.get("inputs.nixpkgs.follows").ok()??;
        (followed == RIKKA_NIXPKGS).then_some(Self::Unstable)
    }

    #[must_use]
    pub fn from_url(url: &str) -> Option<Self> {
        let branch = url.rsplit('/').next()?;
        let named = branch.strip_prefix("nixos-").unwrap_or(branch);
        match named {
            "unstable" | "unstable-small" => Some(Self::Unstable),
            release if release.starts_with(|ch: char| ch.is_ascii_digit()) => {
                Some(Self::Release(release.to_owned()))
            }
            _ => None,
        }
    }

    #[must_use]
    pub fn from_version(version: &str) -> Option<Self> {
        let version = version.trim();
        if version.contains("pre") || version.contains("beta") {
            return Some(Self::Unstable);
        }
        let mut parts = version.split('.');
        let (major, minor) = (parts.next()?, parts.next()?);
        if !major.chars().all(|ch| ch.is_ascii_digit())
            || !minor.chars().all(|ch| ch.is_ascii_digit())
        {
            return None;
        }
        Some(Self::Release(format!("{major}.{minor}")))
    }

    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Unstable => "unstable",
            Self::Release(release) => release,
        }
    }

    #[must_use]
    pub fn flake_url(&self) -> String {
        match self {
            Self::Unstable => "github:NixOS/nixpkgs/nixos-unstable".to_owned(),
            Self::Release(release) => format!("github:NixOS/nixpkgs/nixos-{release}"),
        }
    }
}
