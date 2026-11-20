use rnix::SyntaxKind;

use crate::error::{Error, Result};
use crate::expr::Expr;
use crate::lambda::{Formals, Param};
use crate::name;
use crate::set::{Entry, Set};

pub(crate) fn writable(expr: &Expr) -> Result<()> {
    match expr {
        Expr::Null | Expr::Bool(_) | Expr::String(_) => Ok(()),
        Expr::Int(value) if *value == i64::MIN => Err(Error::Number(value.to_string())),
        Expr::Int(_) => Ok(()),
        Expr::Float(value) if !value.is_finite() => Err(Error::Number(value.to_string())),
        Expr::Float(_) => Ok(()),
        Expr::Path(text) => path(text),
        Expr::List(items) => items.iter().try_for_each(writable),
        Expr::Set(set) => self::set(set),
        Expr::Reference(path) => identifier(path.head()),
        Expr::Apply { function, argument } => {
            writable(function)?;
            writable(argument)
        }
        Expr::With { scope, body } => {
            writable(scope)?;
            writable(body)
        }
        Expr::Lambda { param, body } => {
            self::param(param)?;
            writable(body)
        }
        Expr::Let { bindings, body } => {
            self::set(bindings)?;
            writable(body)
        }
    }
}

fn set(set: &Set) -> Result<()> {
    if let Some(fault) = set.fault() {
        return Err(fault.clone());
    }
    set.entries().iter().try_for_each(|entry| match entry {
        Entry::Binding { value, .. } => writable(value),
        Entry::Dynamic { key, value } => {
            writable(key)?;
            writable(value)
        }
        Entry::Inherit { from, names } => {
            if names.is_empty() {
                return Err(Error::Identifier(String::new()));
            }
            from.iter().try_for_each(writable)?;
            names.iter().try_for_each(|name| identifier(name))
        }
        Entry::Comment(_) | Entry::Blank => Ok(()),
    })
}

fn param(param: &Param) -> Result<()> {
    match param {
        Param::Name(name) => identifier(name),
        Param::Formals(formals) => self::formals(formals),
    }
}

fn formals(formals: &Formals) -> Result<()> {
    formals.bound_name().map_or(Ok(()), identifier)?;
    formals.formals().iter().try_for_each(|formal| {
        identifier(&formal.name)?;
        formal.default.iter().try_for_each(writable)
    })
}

fn identifier(name: &str) -> Result<()> {
    match name::is_identifier(name) {
        true => Ok(()),
        false => Err(Error::Identifier(name.to_owned())),
    }
}

fn path(text: &str) -> Result<()> {
    let mut tokens = rnix::tokenize(text);
    let single = match (tokens.next(), tokens.next()) {
        (Some((kind, token)), None) => {
            token == text
                && matches!(
                    kind,
                    SyntaxKind::TOKEN_PATH_ABS
                        | SyntaxKind::TOKEN_PATH_REL
                        | SyntaxKind::TOKEN_PATH_HOME
                        | SyntaxKind::TOKEN_PATH_SEARCH
                )
        }
        _ => false,
    };
    match single {
        true => Ok(()),
        false => Err(Error::Path(text.to_owned())),
    }
}
