use std::path::{Path, PathBuf};

use rikka_file_operations::Address;

pub const SCHEME_FILE: &str = "file";
pub const SCHEME_TRASH: &str = "trash";
pub const SCHEME_ARCHIVE: &str = "archive";
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Location {
    uri: String,
    scheme_end: usize,
}

impl Location {
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("/"))
                .join(path)
        };
        Self::from_scheme_and_path(SCHEME_FILE, absolute)
    }

    pub fn from_scheme_and_path(scheme: &str, path: impl AsRef<Path>) -> Self {
        let encoded = encode_path(&normalize(path.as_ref()));
        let uri = format!("{}://{encoded}", scheme.to_ascii_lowercase());
        let scheme_end = scheme.len();
        Self { uri, scheme_end }
    }

    pub fn root_of(scheme: &str) -> Self {
        Self::from_scheme_and_path(scheme, "/")
    }

    pub fn parse(uri: &str) -> Option<Self> {
        let scheme_end = uri.find(':')?;
        let scheme = &uri[..scheme_end];
        if !scheme.starts_with(|first: char| first.is_ascii_alphabetic())
            || !scheme
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b))
        {
            return None;
        }
        let rest = &uri[scheme_end + 1..];
        let (authority, path) = match rest.strip_prefix("//") {
            Some(after) => after
                .find('/')
                .map_or((after, "/"), |slash| after.split_at(slash)),
            None => ("", rest),
        };
        let elsewhere = scheme.eq_ignore_ascii_case(SCHEME_FILE)
            && !(authority.is_empty() || authority.eq_ignore_ascii_case("localhost"));
        if elsewhere {
            return None;
        }
        Some(Self::from_scheme_and_path(scheme, decode_path(path)))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.uri
    }

    #[must_use]
    pub fn scheme(&self) -> &str {
        &self.uri[..self.scheme_end]
    }

    #[must_use]
    pub fn is_local(&self) -> bool {
        self.scheme() == SCHEME_FILE
    }

    #[must_use]
    pub fn path(&self) -> PathBuf {
        decode_path(&self.uri[self.scheme_end + 3..])
    }

    #[must_use]
    pub fn to_path(&self) -> Option<PathBuf> {
        self.is_local().then(|| self.path())
    }

    #[must_use]
    pub fn name(&self) -> String {
        let path = self.path();
        path.file_name().map_or_else(
            || self.scheme().to_owned(),
            |name| name.to_string_lossy().into_owned(),
        )
    }

    #[must_use]
    pub fn display(&self) -> String {
        self.to_path().map_or_else(
            || self.uri.clone(),
            |path| path.to_string_lossy().into_owned(),
        )
    }

    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        let path = self.path();
        let parent = path.parent()?;
        Some(Self::from_scheme_and_path(self.scheme(), parent))
    }

    #[must_use]
    pub fn child(&self, segment: impl AsRef<Path>) -> Self {
        Self::from_scheme_and_path(self.scheme(), self.path().join(segment))
    }

    #[must_use]
    pub fn ancestry(&self) -> Vec<Self> {
        let mut chain: Vec<_> = std::iter::successors(Some(self.clone()), Self::parent).collect();
        chain.reverse();
        chain
    }

    #[must_use]
    pub fn contains(&self, other: &Self) -> bool {
        self.scheme() == other.scheme() && other.path().starts_with(self.path())
    }
}

impl Address for Location {
    fn child(&self, name: &str) -> Self {
        Location::child(self, name)
    }

    fn parent(&self) -> Option<Self> {
        Location::parent(self)
    }

    fn contains(&self, other: &Self) -> bool {
        Location::contains(self, other)
    }

    fn name(&self) -> String {
        Location::name(self)
    }
}

impl std::fmt::Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.uri)
    }
}

impl From<&Path> for Location {
    fn from(path: &Path) -> Self {
        Self::from_path(path)
    }
}

impl From<PathBuf> for Location {
    fn from(path: PathBuf) -> Self {
        Self::from_path(path)
    }
}

fn normalize(path: &Path) -> PathBuf {
    let mut out: Vec<&std::ffi::OsStr> = Vec::new();
    for part in path.components() {
        match part {
            std::path::Component::RootDir | std::path::Component::Prefix(_) => out.clear(),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::Normal(name) => out.push(name),
        }
    }
    let mut resolved = PathBuf::from("/");
    resolved.extend(out);
    resolved
}

fn encode_path(path: &Path) -> String {
    rikka_open::encode_path(path)
}

fn decode_path(encoded: &str) -> PathBuf {
    rikka_open::decode_path(encoded)
}
