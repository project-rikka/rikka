use crate::error::Error;
use crate::expr::Expr;
use crate::path::{AttrPath, IntoAttrPath};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Set {
    entries: Vec<Entry>,
    fault: Option<Error>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Binding {
        path: AttrPath,
        value: Expr,
    },
    Dynamic {
        key: Expr,
        value: Expr,
    },
    Inherit {
        from: Option<Expr>,
        names: Vec<String>,
    },
    Comment(String),
    Blank,
}

impl Set {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn bind(mut self, path: impl IntoAttrPath, value: impl Into<Expr>) -> Self {
        match path.into_attr_path() {
            Ok(path) => self.entries.push(Entry::Binding {
                path,
                value: value.into(),
            }),
            Err(error) => {
                self.fault.get_or_insert(error);
            }
        }
        self
    }

    #[must_use]
    pub fn bind_dynamic(mut self, key: impl Into<Expr>, value: impl Into<Expr>) -> Self {
        self.entries.push(Entry::Dynamic {
            key: key.into(),
            value: value.into(),
        });
        self
    }

    #[must_use]
    pub fn inherit<S: Into<String>>(mut self, names: impl IntoIterator<Item = S>) -> Self {
        self.entries.push(Entry::Inherit {
            from: None,
            names: names.into_iter().map(Into::into).collect(),
        });
        self
    }

    #[must_use]
    pub fn comment(mut self, text: impl Into<String>) -> Self {
        self.entries.push(Entry::Comment(text.into()));
        self
    }

    #[must_use]
    pub fn blank(mut self) -> Self {
        self.entries.push(Entry::Blank);
        self
    }

    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.entries.iter().any(|entry| {
            matches!(
                entry,
                Entry::Binding { .. } | Entry::Dynamic { .. } | Entry::Inherit { .. }
            )
        })
    }

    #[must_use]
    pub fn get(&self, path: &AttrPath) -> Option<&Expr> {
        self.entries.iter().find_map(|entry| {
            let Entry::Binding { path: bound, value } = entry else {
                return None;
            };
            if bound == path {
                return Some(value);
            }
            let rest = path.strip_prefix(bound)?;
            value.as_set()?.get(&rest)
        })
    }

    pub(crate) fn fault(&self) -> Option<&Error> {
        self.fault.as_ref()
    }

    pub(crate) fn push(&mut self, entry: Entry) {
        self.entries.push(entry);
    }
}
