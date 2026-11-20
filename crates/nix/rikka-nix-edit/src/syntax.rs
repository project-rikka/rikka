use rnix::ast::{self, Attr, Expr, InterpolPart};
use rnix::{Root, SyntaxKind, TextRange};
use rowan::ast::AstNode;

use crate::error::{Error, Result};

pub(crate) fn parse(source: &str) -> Result<rnix::Parse<Root>> {
    let parsed = Root::parse(source);
    match parsed.errors().first() {
        None => Ok(parsed),
        Some(error) => Err(syntax_error(source, error)),
    }
}

fn syntax_error(source: &str, error: &rnix::ParseError) -> Error {
    use rnix::ParseError as Parse;
    let at = match error {
        Parse::Unexpected(range)
        | Parse::UnexpectedExtra(range)
        | Parse::UnexpectedWanted(_, range, _)
        | Parse::UnexpectedDoubleBind(range)
        | Parse::DuplicatedArgs(range, _) => usize::from(range.start()),
        _ => source.len(),
    };
    let before = &source[..at.min(source.len())];
    let line = before.matches('\n').count() + 1;
    let column = before
        .rsplit('\n')
        .next()
        .map_or(0, |line| line.chars().count())
        + 1;
    Error::Syntax {
        line,
        column,
        message: error.to_string(),
    }
}

pub(crate) fn strip_parens(expr: Expr) -> Expr {
    let mut expr = expr;
    loop {
        expr = match expr {
            Expr::Paren(paren) => match paren.expr() {
                Some(inner) => inner,
                None => return Expr::Paren(paren),
            },
            other => return other,
        };
    }
}

pub(crate) fn body(root: &Root) -> Result<ast::AttrSet> {
    let mut expr = root.expr().ok_or(Error::NotAttrSet)?;
    loop {
        expr = match strip_parens(expr) {
            Expr::Lambda(lambda) => lambda.body().ok_or(Error::NotAttrSet)?,
            Expr::AttrSet(set) => return Ok(set),
            _ => return Err(Error::NotAttrSet),
        };
    }
}

pub(crate) fn pattern(root: &Root) -> Result<ast::Pattern> {
    let Some(Expr::Lambda(lambda)) = root.expr().map(strip_parens) else {
        return Err(Error::NotFunction);
    };
    match lambda.param() {
        Some(ast::Param::Pattern(pattern)) => Ok(pattern),
        _ => Err(Error::NotFunction),
    }
}

pub(crate) fn holds_comment(node: &rnix::SyntaxNode) -> bool {
    node.descendants_with_tokens()
        .any(|element| element.kind() == SyntaxKind::TOKEN_COMMENT)
}

pub(crate) fn keys(attrpath: &ast::Attrpath) -> Option<Vec<String>> {
    attrpath.attrs().map(|attr| name(&attr)).collect()
}

pub(crate) fn name(attr: &Attr) -> Option<String> {
    match attr {
        Attr::Ident(ident) => Some(ident.syntax().text().to_string()),
        Attr::Str(text) => literal(text),
        Attr::Dynamic(_) => None,
    }
}

pub(crate) fn literal(text: &ast::Str) -> Option<String> {
    let mut out = String::new();
    for part in text.normalized_parts() {
        match part {
            InterpolPart::Literal(run) => out.push_str(&run),
            InterpolPart::Interpolation(_) => return None,
        }
    }
    Some(out)
}

pub(crate) fn describe_expr(expr: &Expr) -> &'static str {
    match expr {
        Expr::Apply(_) => "a function application",
        Expr::Assert(_) => "an assertion",
        Expr::Error(_) | Expr::Root(_) => "not an expression",
        Expr::IfElse(_) => "a conditional",
        Expr::Select(_) => "a selection",
        Expr::Str(_) => "a string",
        Expr::PathAbs(_) | Expr::PathRel(_) | Expr::PathHome(_) | Expr::PathSearch(_) => "a path",
        Expr::Literal(_) => "a number",
        Expr::Lambda(_) => "a function",
        Expr::LegacyLet(_) | Expr::LetIn(_) => "a let expression",
        Expr::List(_) => "a list",
        Expr::BinOp(_) | Expr::UnaryOp(_) => "an operation",
        Expr::Paren(_) => "a parenthesized expression",
        Expr::AttrSet(_) => "an attribute set",
        Expr::Ident(_) => "a variable",
        Expr::With(_) => "a with expression",
        Expr::HasAttr(_) => "an attribute test",
        Expr::CurPos(_) => "a position",
    }
}

pub(crate) fn closing_brace(set: &ast::AttrSet) -> Option<TextRange> {
    set.syntax()
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() == SyntaxKind::TOKEN_R_BRACE)
        .last()
        .map(|token| token.text_range())
}

pub(crate) fn opening_brace(set: &ast::AttrSet) -> Option<TextRange> {
    set.syntax()
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == SyntaxKind::TOKEN_L_BRACE)
        .map(|token| token.text_range())
}
