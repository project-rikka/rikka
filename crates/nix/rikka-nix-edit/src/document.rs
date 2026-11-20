use std::fmt;
use std::ops::Range;

use rnix::Root;
use rnix::ast::{self, HasEntry};
use rowan::ast::AstNode;

use crate::check;
use crate::error::{Error, Result};
use crate::expr::Expr;
use crate::from_nix::FromNix;
use crate::locate::{self, Found};
use crate::name;
use crate::path::{AttrPath, IntoAttrPath};
use crate::print;
use crate::read;
use crate::set::{Entry, Set};
use crate::syntax;
use crate::text;

#[derive(Clone)]
pub struct Document {
    source: String,
    tree: rnix::Parse<Root>,
}

impl Document {
    pub fn parse(source: impl Into<String>) -> Result<Self> {
        let source = source.into();
        let tree = syntax::parse(&source)?;
        Ok(Self { source, tree })
    }

    pub fn new(expr: impl Into<Expr>) -> Result<Self> {
        Self::render(None, &expr.into())
    }

    pub fn with_header(header: &str, expr: impl Into<Expr>) -> Result<Self> {
        Self::render(Some(header), &expr.into())
    }

    fn render(header: Option<&str>, expr: &Expr) -> Result<Self> {
        check::writable(expr)?;
        Self::parse(print::document(header, expr))
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn get<T: FromNix>(&self, path: impl IntoAttrPath) -> Result<Option<T>> {
        let path = path.into_attr_path()?;
        let found = self.find(&path)?;
        let opaque = |found| Error::Opaque {
            path: path.clone(),
            found,
        };
        let value = match found.exact() {
            Some(pair) => value(pair).map_err(opaque)?,
            None if found.covered.is_empty() => return Ok(None),
            None => {
                let mut merged = Set::new();
                for covered in &found.covered {
                    let value = value(&covered.pair).map_err(opaque)?;
                    if !covered.beyond.is_empty() {
                        merged.push(Entry::Binding {
                            path: AttrPath::from_segments(covered.beyond.clone()),
                            value,
                        });
                        continue;
                    }
                    let Expr::Set(inner) = value else {
                        return Err(Error::Mismatch {
                            path,
                            expected: Set::EXPECTED,
                        });
                    };
                    for entry in inner.entries() {
                        merged.push(entry.clone());
                    }
                }
                Expr::Set(merged)
            }
        };
        T::from_nix(value).map(Some).ok_or(Error::Mismatch {
            path,
            expected: T::EXPECTED,
        })
    }

    pub fn set(&mut self, path: impl IntoAttrPath, value: impl Into<Expr>) -> Result<()> {
        let path = path.into_attr_path()?;
        let value = value.into();
        check::writable(&value)?;
        let found = self.find(&path)?;
        if let Some(pair) = found.exact() {
            let next = self.replaced(pair, &value);
            return self.commit(next);
        }
        let mut next = self.clone();
        next.strip(&path)?;
        next.insert(&path, &value)?;
        *self = next;
        Ok(())
    }

    pub fn remove(&mut self, path: impl IntoAttrPath) -> Result<()> {
        let path = path.into_attr_path()?;
        let mut next = self.clone();
        if !next.strip(&path)? {
            return Ok(());
        }
        for length in (1..path.segments().len()).rev() {
            let parent = AttrPath::from_segments(path.segments()[..length].to_vec());
            let found = next.find(&parent)?;
            if found.covered.is_empty() {
                continue;
            }
            let emptied = found
                .exact()
                .and_then(ast::AttrpathValue::value)
                .is_some_and(|value| match syntax::strip_parens(value) {
                    ast::Expr::AttrSet(set) => next.bare(&set).is_some(),
                    _ => false,
                });
            if !emptied {
                break;
            }
            next.strip(&parent)?;
        }
        next.close_if_empty(&path)?;
        *self = next;
        Ok(())
    }

    pub fn include(&mut self, path: impl IntoAttrPath, item: impl Into<Expr>) -> Result<()> {
        let path = path.into_attr_path()?;
        let item = item.into();
        let mut items = self.get::<Vec<Expr>>(&path)?.unwrap_or_default();
        if items.contains(&item) {
            return Ok(());
        }
        items.push(item);
        self.set(path, Expr::List(items))
    }

    pub fn exclude(&mut self, path: impl IntoAttrPath, item: impl Into<Expr>) -> Result<()> {
        let path = path.into_attr_path()?;
        let item = item.into();
        let Some(items) = self.get::<Vec<Expr>>(&path)? else {
            return Ok(());
        };
        if !items.contains(&item) {
            return Ok(());
        }
        let kept: Vec<Expr> = items.into_iter().filter(|kept| *kept != item).collect();
        match kept.is_empty() {
            true => self.remove(path),
            false => self.set(path, Expr::List(kept)),
        }
    }

    fn close_if_empty(&mut self, path: &AttrPath) -> Result<()> {
        let set = self.find(path)?.container;
        let Some(inside) = self
            .bare(&set)
            .filter(|inside| &self.source[inside.clone()] != " ")
        else {
            return Ok(());
        };
        let mut next = self.source.clone();
        next.replace_range(inside, " ");
        self.commit(next)
    }

    fn bare(&self, set: &ast::AttrSet) -> Option<Range<usize>> {
        let open = syntax::opening_brace(set)?;
        let close = syntax::closing_brace(set)?;
        let inside = usize::from(open.end())..usize::from(close.start());
        self.source[inside.clone()]
            .trim()
            .is_empty()
            .then_some(inside)
    }

    pub fn add_formal(&mut self, name: &str) -> Result<()> {
        if !name::is_identifier(name) {
            return Err(Error::Identifier(name.to_owned()));
        }
        let pattern = syntax::pattern(&self.tree.tree())?;
        if syntax::holds_comment(pattern.syntax()) {
            return Err(Error::OpaquePattern("a comment"));
        }
        let formals = read::formals(&pattern).map_err(Error::OpaquePattern)?;
        if formals.formals().iter().any(|formal| formal.name == name) {
            return Ok(());
        }
        let at = span(pattern.syntax());
        let line = text::line_start(&self.source, at.start);
        let mut written = self.source[line..at.start].to_owned();
        print::formals(
            &mut written,
            &formals.required(name),
            text::indent(&self.source, at.start),
        );
        let mut next = self.source.clone();
        next.replace_range(at.clone(), &written[at.start - line..]);
        self.commit(next)
    }

    pub fn paths(&self) -> Result<Vec<AttrPath>> {
        let mut paths = Vec::new();
        locate::leaves(&self.body()?, &[], &mut paths);
        Ok(paths)
    }

    fn body(&self) -> Result<ast::AttrSet> {
        syntax::body(&self.tree.tree())
    }

    fn find(&self, path: &AttrPath) -> Result<Found> {
        locate::find(self.body()?, path)
    }

    fn commit(&mut self, source: String) -> Result<()> {
        *self = Self::parse(source)?;
        Ok(())
    }

    fn replaced(&self, pair: &ast::AttrpathValue, value: &Expr) -> String {
        let at = span(pair.syntax());
        let key = pair
            .attrpath()
            .map(|attrpath| attrpath.syntax().text().to_string())
            .unwrap_or_default();
        let line = text::line_start(&self.source, at.start);
        let mut written = self.source[line..at.start].to_owned();
        print::binding_into(
            &mut written,
            &key,
            value,
            text::indent(&self.source, at.start),
        );
        let mut next = self.source.clone();
        next.replace_range(at.clone(), &written[at.start - line..]);
        next
    }

    fn strip(&mut self, path: &AttrPath) -> Result<bool> {
        let found = self.find(path)?;
        if found.covered.is_empty() {
            return Ok(false);
        }
        let spans = found
            .covered
            .iter()
            .map(|covered| span(covered.pair.syntax()))
            .collect();
        let mut next = self.source.clone();
        for span in text::joined(&self.source, spans).into_iter().rev() {
            let removal = text::removal(&self.source, span);
            next.replace_range(removal.range.clone(), "");
            if removal.lines {
                text::tidy(&mut next, removal.range.start);
            }
        }
        self.commit(next)?;
        Ok(true)
    }

    fn insert(&mut self, path: &AttrPath, value: &Expr) -> Result<()> {
        let found = self.find(path)?;
        let rest = AttrPath::from_segments(path.segments()[found.depth..].to_vec());
        let set = found.container;
        let (Some(open), Some(close)) = (syntax::opening_brace(&set), syntax::closing_brace(&set))
        else {
            return Err(Error::NotAttrSet);
        };
        let open = usize::from(open.start());
        let close = usize::from(close.start());
        let source = &self.source;
        let binding = |indent: usize| {
            let mut written = text::spaces(indent);
            print::binding_into(&mut written, &rest.to_string(), value, indent);
            written
        };

        let mut next = source.clone();
        if !source[open..close].contains('\n') {
            let base = text::indent(source, open);
            let mut written = String::from("{");
            for entry in set.entries() {
                written.push('\n');
                written.push_str(&text::spaces(base + 2));
                written.push_str(&entry.syntax().text().to_string());
            }
            written.push('\n');
            written.push_str(&binding(base + 2));
            written.push('\n');
            written.push_str(&text::spaces(base));
            written.push('}');
            next.replace_range(open..close + 1, &written);
            return self.commit(next);
        }

        let indent = set
            .entries()
            .last()
            .map(|entry| usize::from(entry.syntax().text_range().start()))
            .filter(|start| text::heads_line(source, *start))
            .map_or_else(
                || text::indent(source, open) + 2,
                |start| text::indent(source, start),
            );
        match text::heads_line(source, close) {
            true => {
                let at = text::line_start(source, close);
                next.insert_str(at, &format!("{}\n", binding(indent)));
            }
            false => {
                let trimmed = source[..close].trim_end_matches([' ', '\t']).len();
                let written = format!(
                    "\n{}\n{}",
                    binding(indent),
                    text::spaces(text::indent(source, open))
                );
                next.replace_range(trimmed..close, &written);
            }
        }
        self.commit(next)
    }
}

fn value(pair: &ast::AttrpathValue) -> Result<Expr, &'static str> {
    read::expr(pair.value().ok_or("not an expression")?)
}

fn span(node: &rnix::SyntaxNode) -> Range<usize> {
    let range = node.text_range();
    usize::from(range.start())..usize::from(range.end())
}

impl PartialEq for Document {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
    }
}

impl Eq for Document {}

impl fmt::Debug for Document {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Document").field(&self.source).finish()
    }
}

impl fmt::Display for Document {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.source)
    }
}
