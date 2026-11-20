use std::borrow::Cow;
use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Label {
    text: Arc<str>,
    key: Option<Arc<str>>,
}

impl Label {
    pub fn translated(key: &str) -> Self {
        let key = Arc::<str>::from(key);
        Self {
            text: crate::global::tr(&key),
            key: Some(key),
        }
    }

    pub fn translated_or(key: &str, fallback: &'static str) -> Self {
        let key = Arc::<str>::from(key);
        Self {
            text: crate::global::translate(&key).unwrap_or_else(|| Arc::from(fallback)),
            key: Some(key),
        }
    }

    pub fn formatted(key: &str, args: &crate::Args<'_>) -> Self {
        Self {
            text: crate::global::tr_with(key, args),
            key: None,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }

    #[must_use]
    pub fn retranslated(&self) -> Self {
        match &self.key {
            Some(key) => Self {
                text: crate::global::translate(key).unwrap_or_else(|| Arc::clone(&self.text)),
                key: Some(Arc::clone(key)),
            },
            None => self.clone(),
        }
    }
}

macro_rules! verbatim_from {
    ($($ty:ty),+ $(,)?) => {
        $(impl From<$ty> for Label {
            fn from(text: $ty) -> Self {
                Self {
                    text: Arc::<str>::from(text),
                    key: None,
                }
            }
        })+
    };
}

verbatim_from!(&str, String, Box<str>, Cow<'_, str>, Arc<str>);

impl From<&String> for Label {
    fn from(text: &String) -> Self {
        Self::from(text.as_str())
    }
}

impl From<Label> for Arc<str> {
    fn from(label: Label) -> Self {
        label.text
    }
}

impl From<Label> for String {
    fn from(label: Label) -> Self {
        label.text.to_string()
    }
}

impl Default for Label {
    fn default() -> Self {
        Self::from("")
    }
}

impl Deref for Label {
    type Target = str;

    fn deref(&self) -> &str {
        &self.text
    }
}

impl AsRef<str> for Label {
    fn as_ref(&self) -> &str {
        &self.text
    }
}

impl std::fmt::Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}
