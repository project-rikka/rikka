use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::{Arc, LazyLock};

use rustc_hash::FxHasher;

#[derive(Clone)]
pub struct Text {
    content: Arc<str>,
    hash: u64,
}

impl Text {
    pub fn new(content: impl Into<Arc<str>>) -> Self {
        let content = content.into();
        let hash = digest(&content);
        Self { content, hash }
    }

    #[must_use]
    pub fn empty() -> Self {
        static EMPTY: LazyLock<Text> = LazyLock::new(|| Text::new(""));
        EMPTY.clone()
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.content
    }

    #[must_use]
    pub fn digest(&self) -> u64 {
        self.hash
    }

    #[must_use]
    pub fn same(&self, other: &Text) -> bool {
        Arc::ptr_eq(&self.content, &other.content)
    }
}

fn digest(content: &str) -> u64 {
    let mut hasher = FxHasher::default();
    content.hash(&mut hasher);
    hasher.finish()
}

impl PartialEq for Text {
    fn eq(&self, other: &Self) -> bool {
        self.same(other) || (self.hash == other.hash && *self.content == *other.content)
    }
}

impl Eq for Text {}

impl Hash for Text {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash);
    }
}

impl PartialEq<str> for Text {
    fn eq(&self, other: &str) -> bool {
        *self.content == *other
    }
}

impl PartialEq<&str> for Text {
    fn eq(&self, other: &&str) -> bool {
        *self.content == **other
    }
}

impl Deref for Text {
    type Target = str;

    fn deref(&self) -> &str {
        &self.content
    }
}

impl AsRef<str> for Text {
    fn as_ref(&self) -> &str {
        &self.content
    }
}

impl Default for Text {
    fn default() -> Self {
        Self::empty()
    }
}

impl fmt::Debug for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.content, f)
    }
}

impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&*self.content, f)
    }
}

impl From<Arc<str>> for Text {
    fn from(content: Arc<str>) -> Self {
        Self::new(content)
    }
}

impl From<&Arc<str>> for Text {
    fn from(content: &Arc<str>) -> Self {
        Self::new(Arc::clone(content))
    }
}

impl From<String> for Text {
    fn from(content: String) -> Self {
        Self::new(content)
    }
}

impl From<&String> for Text {
    fn from(content: &String) -> Self {
        Self::new(content.as_str())
    }
}

impl From<&str> for Text {
    fn from(content: &str) -> Self {
        Self::new(content)
    }
}

impl From<Box<str>> for Text {
    fn from(content: Box<str>) -> Self {
        Self::new(content)
    }
}

impl From<std::borrow::Cow<'_, str>> for Text {
    fn from(content: std::borrow::Cow<'_, str>) -> Self {
        Self::new(content)
    }
}

impl From<char> for Text {
    fn from(content: char) -> Self {
        Self::new(content.to_string())
    }
}

impl From<&Text> for Text {
    fn from(content: &Text) -> Self {
        content.clone()
    }
}

impl From<Text> for Arc<str> {
    fn from(text: Text) -> Self {
        text.content
    }
}
