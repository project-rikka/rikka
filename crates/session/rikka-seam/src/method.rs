use std::sync::Arc;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Method(Arc<str>);

impl Method {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for Method {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl From<&str> for Method {
    fn from(name: &str) -> Self {
        Self(Arc::from(name))
    }
}

impl From<String> for Method {
    fn from(name: String) -> Self {
        Self(Arc::from(name))
    }
}

impl From<&Method> for Method {
    fn from(method: &Method) -> Self {
        method.clone()
    }
}

impl PartialEq<&str> for Method {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::fmt::Debug for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_str(), f)
    }
}
