use crate::expr::Expr;

#[derive(Clone, Debug, PartialEq)]
pub enum Param {
    Name(String),
    Formals(Formals),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Formals {
    formals: Vec<Formal>,
    ellipsis: bool,
    bound: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Formal {
    pub name: String,
    pub default: Option<Expr>,
}

impl Formals {
    #[must_use]
    pub fn new<S: Into<String>>(names: impl IntoIterator<Item = S>) -> Self {
        Self {
            formals: names
                .into_iter()
                .map(|name| Formal {
                    name: name.into(),
                    default: None,
                })
                .collect(),
            ellipsis: false,
            bound: None,
        }
    }

    #[must_use]
    pub fn required(mut self, name: impl Into<String>) -> Self {
        self.formals.push(Formal {
            name: name.into(),
            default: None,
        });
        self
    }

    #[must_use]
    pub fn optional(mut self, name: impl Into<String>, default: impl Into<Expr>) -> Self {
        self.formals.push(Formal {
            name: name.into(),
            default: Some(default.into()),
        });
        self
    }

    #[must_use]
    pub fn ellipsis(mut self) -> Self {
        self.ellipsis = true;
        self
    }

    #[must_use]
    pub fn bound(mut self, name: impl Into<String>) -> Self {
        self.bound = Some(name.into());
        self
    }

    #[must_use]
    pub fn formals(&self) -> &[Formal] {
        &self.formals
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.ellipsis
    }

    #[must_use]
    pub fn bound_name(&self) -> Option<&str> {
        self.bound.as_deref()
    }
}

impl From<Formals> for Param {
    fn from(formals: Formals) -> Self {
        Self::Formals(formals)
    }
}

impl From<&str> for Param {
    fn from(name: &str) -> Self {
        Self::Name(name.to_owned())
    }
}

impl From<String> for Param {
    fn from(name: String) -> Self {
        Self::Name(name)
    }
}
