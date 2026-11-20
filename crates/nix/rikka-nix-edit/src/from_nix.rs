use crate::expr::Expr;
use crate::path::AttrPath;
use crate::set::Set;

pub trait FromNix: Sized {
    const EXPECTED: &'static str;

    fn from_nix(expr: Expr) -> Option<Self>;
}

impl FromNix for Expr {
    const EXPECTED: &'static str = "an expression";

    fn from_nix(expr: Expr) -> Option<Self> {
        Some(expr)
    }
}

impl FromNix for bool {
    const EXPECTED: &'static str = "a boolean";

    fn from_nix(expr: Expr) -> Option<Self> {
        expr.as_bool()
    }
}

impl FromNix for i64 {
    const EXPECTED: &'static str = "an integer";

    fn from_nix(expr: Expr) -> Option<Self> {
        expr.as_int()
    }
}

impl FromNix for f64 {
    const EXPECTED: &'static str = "a number";

    fn from_nix(expr: Expr) -> Option<Self> {
        match expr {
            Expr::Float(value) => Some(value),
            #[expect(
                clippy::cast_precision_loss,
                reason = "Nix integers convert to floats as Nix itself does"
            )]
            Expr::Int(value) => Some(value as f64),
            _ => None,
        }
    }
}

impl FromNix for String {
    const EXPECTED: &'static str = "a string";

    fn from_nix(expr: Expr) -> Option<Self> {
        match expr {
            Expr::String(text) => Some(text),
            _ => None,
        }
    }
}

impl FromNix for Set {
    const EXPECTED: &'static str = "an attribute set";

    fn from_nix(expr: Expr) -> Option<Self> {
        match expr {
            Expr::Set(set) => Some(set),
            _ => None,
        }
    }
}

impl FromNix for AttrPath {
    const EXPECTED: &'static str = "a reference";

    fn from_nix(expr: Expr) -> Option<Self> {
        match expr {
            Expr::Reference(path) => Some(path),
            _ => None,
        }
    }
}

impl<T: FromNix> FromNix for Vec<T> {
    const EXPECTED: &'static str = "a list";

    fn from_nix(expr: Expr) -> Option<Self> {
        match expr {
            Expr::List(items) => items.into_iter().map(T::from_nix).collect(),
            _ => None,
        }
    }
}
