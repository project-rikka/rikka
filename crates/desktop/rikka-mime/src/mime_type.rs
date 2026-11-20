use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MimeType {
    essence: String,
    slash: usize,
}

impl MimeType {
    pub const DIRECTORY: &'static str = "inode/directory";
    pub const SYMLINK: &'static str = "inode/symlink";
    pub const UNKNOWN: &'static str = "application/octet-stream";
    pub const EMPTY: &'static str = "application/x-zerosize";
    pub const TEXT: &'static str = "text/plain";

    pub fn parse(essence: &str) -> Option<Self> {
        let essence = essence.trim();
        let essence = essence.split(';').next().unwrap_or(essence).trim();
        let (kind, subtype) = essence.split_once('/')?;
        if !is_restricted_name(kind) || !is_restricted_name(subtype) {
            return None;
        }
        Some(Self {
            essence: essence.to_ascii_lowercase(),
            slash: kind.len(),
        })
    }

    #[must_use]
    pub fn well_known(essence: &'static str) -> Self {
        Self::parse(essence).expect("a media type written out in source")
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.essence
    }

    #[must_use]
    pub fn kind(&self) -> &str {
        &self.essence[..self.slash]
    }

    #[must_use]
    pub fn subtype(&self) -> &str {
        &self.essence[self.slash + 1..]
    }

    #[must_use]
    pub fn is_directory(&self) -> bool {
        self.essence == Self::DIRECTORY
    }
}

impl fmt::Display for MimeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.essence)
    }
}

fn is_restricted_name(part: &str) -> bool {
    part.bytes()
        .next()
        .is_some_and(|first| first.is_ascii_alphanumeric())
        && part
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$&-^_.+".contains(&byte))
}
