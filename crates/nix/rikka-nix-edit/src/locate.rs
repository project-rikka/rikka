use rnix::ast::{self, Entry, HasEntry};

use crate::error::{Error, Result};
use crate::path::AttrPath;
use crate::syntax;

pub(crate) struct Covered {
    pub(crate) pair: ast::AttrpathValue,
    pub(crate) beyond: Vec<String>,
}

pub(crate) struct Found {
    pub(crate) covered: Vec<Covered>,
    pub(crate) container: ast::AttrSet,
    pub(crate) depth: usize,
}

impl Found {
    pub(crate) fn exact(&self) -> Option<&ast::AttrpathValue> {
        match self.covered.as_slice() {
            [only] if only.beyond.is_empty() => Some(&only.pair),
            _ => None,
        }
    }
}

pub(crate) fn find(body: ast::AttrSet, path: &AttrPath) -> Result<Found> {
    let mut found = Found {
        covered: Vec::new(),
        container: body.clone(),
        depth: 0,
    };
    walk(&body, path, 0, &mut found)?;
    Ok(found)
}

fn walk(set: &ast::AttrSet, path: &AttrPath, depth: usize, found: &mut Found) -> Result<()> {
    if depth > found.depth {
        found.container = set.clone();
        found.depth = depth;
    }
    let rest = &path.segments()[depth..];
    for entry in set.entries() {
        match entry {
            Entry::Inherit(inherit) => {
                if inherit
                    .attrs()
                    .any(|attr| syntax::name(&attr).as_deref() == Some(rest[0].as_str()))
                {
                    return Err(opaque(path, depth + 1, "an inherited name"));
                }
            }
            Entry::AttrpathValue(pair) => {
                let Some(keys) = pair.attrpath().and_then(|attrpath| syntax::keys(&attrpath))
                else {
                    continue;
                };
                let shared = keys.len().min(rest.len());
                if keys[..shared] != rest[..shared] {
                    continue;
                }
                if keys.len() >= rest.len() {
                    found.covered.push(Covered {
                        beyond: keys[rest.len()..].to_vec(),
                        pair,
                    });
                    continue;
                }
                let Some(value) = pair.value() else {
                    continue;
                };
                match syntax::strip_parens(value) {
                    ast::Expr::AttrSet(nested) => walk(&nested, path, depth + keys.len(), found)?,
                    other => {
                        return Err(opaque(
                            path,
                            depth + keys.len(),
                            syntax::describe_expr(&other),
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn opaque(path: &AttrPath, length: usize, found: &'static str) -> Error {
    Error::Opaque {
        path: AttrPath::from_segments(path.segments()[..length].to_vec()),
        found,
    }
}

pub(crate) fn leaves(set: &ast::AttrSet, prefix: &[String], out: &mut Vec<AttrPath>) {
    for entry in set.entries() {
        match entry {
            Entry::Inherit(inherit) => {
                for name in inherit.attrs().filter_map(|attr| syntax::name(&attr)) {
                    out.push(AttrPath::from_segments([prefix, &[name]].concat()));
                }
            }
            Entry::AttrpathValue(pair) => {
                let Some(keys) = pair.attrpath().and_then(|attrpath| syntax::keys(&attrpath))
                else {
                    continue;
                };
                let full = [prefix, keys.as_slice()].concat();
                match pair.value().map(syntax::strip_parens) {
                    Some(ast::Expr::AttrSet(nested)) if nested.entries().next().is_some() => {
                        leaves(&nested, &full, out);
                    }
                    _ => out.push(AttrPath::from_segments(full)),
                }
            }
        }
    }
}
