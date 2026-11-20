use rnix::ast::{self, Entry as SyntaxEntry, HasEntry, LiteralKind, UnaryOpKind};
use rowan::ast::AstNode;

use crate::expr::Expr;
use crate::lambda::{Formals, Param};
use crate::path::AttrPath;
use crate::set::{Entry, Set};
use crate::syntax;

pub(crate) type Read<T> = Result<T, &'static str>;

pub(crate) fn expr(node: ast::Expr) -> Read<Expr> {
    match syntax::strip_parens(node) {
        ast::Expr::Literal(literal) => match literal.kind() {
            LiteralKind::Integer(token) => token.value().map(Expr::Int).map_err(|_| "a number"),
            LiteralKind::Float(token) => token.value().map(Expr::Float).map_err(|_| "a number"),
            LiteralKind::Uri(_) => Err("a URI"),
        },
        ast::Expr::UnaryOp(operation) => negated(&operation),
        ast::Expr::Ident(ident) => Ok(variable(&ident.syntax().text().to_string())),
        ast::Expr::Select(select) => reference(&select),
        ast::Expr::Str(text) => syntax::literal(&text)
            .map(Expr::String)
            .ok_or("an interpolated string"),
        ast::Expr::PathAbs(path) => interpolation_free(path.syntax()),
        ast::Expr::PathRel(path) => interpolation_free(path.syntax()),
        ast::Expr::PathHome(path) => interpolation_free(path.syntax()),
        ast::Expr::PathSearch(path) => Ok(Expr::Path(path.syntax().text().to_string())),
        ast::Expr::List(list) => list.items().map(expr).collect::<Read<_>>().map(Expr::List),
        ast::Expr::AttrSet(set) => self::set(&set).map(Expr::Set),
        ast::Expr::Apply(apply) => Ok(Expr::apply(
            expr(apply.lambda().ok_or("not an expression")?)?,
            expr(apply.argument().ok_or("not an expression")?)?,
        )),
        ast::Expr::With(with) => Ok(Expr::with(
            expr(with.namespace().ok_or("not an expression")?)?,
            expr(with.body().ok_or("not an expression")?)?,
        )),
        ast::Expr::Lambda(lambda) => Ok(Expr::lambda(
            param(lambda.param().ok_or("not an expression")?)?,
            expr(lambda.body().ok_or("not an expression")?)?,
        )),
        other => Err(syntax::describe_expr(&other)),
    }
}

pub(crate) fn set(set: &ast::AttrSet) -> Read<Set> {
    if set.rec_token().is_some() {
        return Err("a recursive attribute set");
    }
    let mut read = Set::new();
    for entry in set.entries() {
        read.push(self::entry(entry)?);
    }
    Ok(read)
}

fn entry(entry: SyntaxEntry) -> Read<Entry> {
    match entry {
        SyntaxEntry::AttrpathValue(pair) => {
            let keys = pair
                .attrpath()
                .and_then(|attrpath| syntax::keys(&attrpath))
                .ok_or("a dynamic attribute")?;
            Ok(Entry::Binding {
                path: AttrPath::from_segments(keys),
                value: expr(pair.value().ok_or("not an expression")?)?,
            })
        }
        SyntaxEntry::Inherit(inherit) => Ok(Entry::Inherit {
            from: inherit
                .from()
                .map(|from| expr(from.expr().ok_or("not an expression")?))
                .transpose()?,
            names: inherit
                .attrs()
                .map(|attr| syntax::name(&attr).ok_or("a dynamic attribute"))
                .collect::<Read<_>>()?,
        }),
    }
}

fn negated(operation: &ast::UnaryOp) -> Read<Expr> {
    if operation.operator() != Some(UnaryOpKind::Negate) {
        return Err("an operation");
    }
    match operation.expr().map(syntax::strip_parens) {
        Some(ast::Expr::Literal(literal)) => match literal.kind() {
            LiteralKind::Integer(token) => token
                .value()
                .ok()
                .and_then(i64::checked_neg)
                .map(Expr::Int)
                .ok_or("a number"),
            LiteralKind::Float(token) => token
                .value()
                .map(|value| Expr::Float(-value))
                .map_err(|_| "a number"),
            LiteralKind::Uri(_) => Err("an operation"),
        },
        _ => Err("an operation"),
    }
}

fn variable(name: &str) -> Expr {
    match name {
        "true" => Expr::Bool(true),
        "false" => Expr::Bool(false),
        "null" => Expr::Null,
        _ => Expr::Reference(AttrPath::from_segments(vec![name.to_owned()])),
    }
}

fn reference(select: &ast::Select) -> Read<Expr> {
    if select.or_token().is_some() {
        return Err("a selection with a default");
    }
    let Some(ast::Expr::Ident(head)) = select.expr().map(syntax::strip_parens) else {
        return Err("a selection");
    };
    let mut segments = vec![head.syntax().text().to_string()];
    let rest = select
        .attrpath()
        .and_then(|attrpath| syntax::keys(&attrpath))
        .ok_or("a dynamic selection")?;
    segments.extend(rest);
    Ok(Expr::Reference(AttrPath::from_segments(segments)))
}

fn interpolation_free(node: &rnix::SyntaxNode) -> Read<Expr> {
    match node.children().next() {
        Some(_) => Err("an interpolated path"),
        None => Ok(Expr::Path(node.text().to_string())),
    }
}

fn param(param: ast::Param) -> Read<Param> {
    match param {
        ast::Param::IdentParam(ident) => ident
            .ident()
            .map(|ident| Param::Name(ident.syntax().text().to_string()))
            .ok_or("not an expression"),
        ast::Param::Pattern(pattern) => formals(&pattern).map(Param::Formals),
    }
}

pub(crate) fn formals(pattern: &ast::Pattern) -> Read<Formals> {
    let mut formals = Formals::default();
    for entry in pattern.pat_entries() {
        let name = entry
            .ident()
            .map(|ident| ident.syntax().text().to_string())
            .ok_or("not an expression")?;
        formals = match entry.default() {
            Some(default) => formals.optional(name, expr(default)?),
            None => formals.required(name),
        };
    }
    if pattern.ellipsis_token().is_some() {
        formals = formals.ellipsis();
    }
    if let Some(bound) = pattern
        .pat_bind()
        .and_then(|bind| bind.ident())
        .map(|ident| ident.syntax().text().to_string())
    {
        formals = formals.bound(bound);
    }
    Ok(formals)
}
