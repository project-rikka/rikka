use crate::error::Result;
use crate::lambda::Param;
use crate::path::{AttrPath, IntoAttrPath};
use crate::set::Set;

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Path(String),
    List(Vec<Expr>),
    Set(Set),
    Reference(AttrPath),
    Apply {
        function: Box<Expr>,
        argument: Box<Expr>,
    },
    Lambda {
        param: Param,
        body: Box<Expr>,
    },
    With {
        scope: Box<Expr>,
        body: Box<Expr>,
    },
    Let {
        bindings: Set,
        body: Box<Expr>,
    },
}

impl Expr {
    pub fn reference(path: impl IntoAttrPath) -> Result<Self> {
        path.into_attr_path().map(Self::Reference)
    }

    #[must_use]
    pub fn path(text: impl Into<String>) -> Self {
        Self::Path(text.into())
    }

    #[must_use]
    pub fn list<T: Into<Self>>(items: impl IntoIterator<Item = T>) -> Self {
        Self::List(items.into_iter().map(Into::into).collect())
    }

    #[must_use]
    pub fn apply(function: impl Into<Self>, argument: impl Into<Self>) -> Self {
        Self::Apply {
            function: Box::new(function.into()),
            argument: Box::new(argument.into()),
        }
    }

    #[must_use]
    pub fn lambda(param: impl Into<Param>, body: impl Into<Self>) -> Self {
        Self::Lambda {
            param: param.into(),
            body: Box::new(body.into()),
        }
    }

    #[must_use]
    pub fn with(scope: impl Into<Self>, body: impl Into<Self>) -> Self {
        Self::With {
            scope: Box::new(scope.into()),
            body: Box::new(body.into()),
        }
    }

    #[must_use]
    pub fn let_in(bindings: Set, body: impl Into<Self>) -> Self {
        Self::Let {
            bindings,
            body: Box::new(body.into()),
        }
    }

    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(value) => Some(*value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_set(&self) -> Option<&Set> {
        match self {
            Self::Set(set) => Some(set),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_reference(&self) -> Option<&AttrPath> {
        match self {
            Self::Reference(path) => Some(path),
            _ => None,
        }
    }
}

impl From<bool> for Expr {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for Expr {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<i32> for Expr {
    fn from(value: i32) -> Self {
        Self::Int(value.into())
    }
}

impl From<u32> for Expr {
    fn from(value: u32) -> Self {
        Self::Int(value.into())
    }
}

impl From<u8> for Expr {
    fn from(value: u8) -> Self {
        Self::Int(value.into())
    }
}

impl From<f64> for Expr {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<&str> for Expr {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<&String> for Expr {
    fn from(value: &String) -> Self {
        Self::String(value.clone())
    }
}

impl From<String> for Expr {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<Set> for Expr {
    fn from(set: Set) -> Self {
        Self::Set(set)
    }
}

impl<T: Into<Self>> From<Vec<T>> for Expr {
    fn from(items: Vec<T>) -> Self {
        Self::list(items)
    }
}

impl<T: Into<Self>, const N: usize> From<[T; N]> for Expr {
    fn from(items: [T; N]) -> Self {
        Self::list(items)
    }
}
