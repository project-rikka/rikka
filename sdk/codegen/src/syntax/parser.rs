use crate::diagnostic::{Diagnostic, Result, Span};
use crate::syntax::ast::{
    Argument, Attribute, Callback, Class, ClosurePart, Constant, DeclarationKind, DocLine, Enum,
    Field, File, Function, Item, ItemKind, Lifetime, Literal, LiteralKind, Member, MemberKind,
    Name, Param, Struct, TypeExpr, Variant,
};
use crate::syntax::lexer::{Symbol, Token, TokenKind};

pub(crate) fn parse(tokens: Vec<Token>) -> Result<File> {
    let mut parser = Parser {
        tokens,
        position: 0,
    };
    parser.file()
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.position]
    }

    fn peek_at(&self, offset: usize) -> &Token {
        let index = (self.position + offset).min(self.tokens.len() - 1);
        &self.tokens[index]
    }

    fn next(&mut self) -> Token {
        let token = self.tokens[self.position].clone();
        if token.kind != TokenKind::End {
            self.position += 1;
        }
        token
    }

    fn unexpected(&self, expected: &str) -> Diagnostic {
        let token = self.peek();
        let found = match &token.kind {
            TokenKind::Identifier(text) => format!("`{text}`"),
            TokenKind::Integer(_) | TokenKind::Float(_) => "a number".to_owned(),
            TokenKind::String(_) => "a string".to_owned(),
            TokenKind::Doc(_) => "a doc comment".to_owned(),
            TokenKind::Symbol(symbol) => format!("`{}`", symbol.text()),
            TokenKind::End => "the end of the file".to_owned(),
        };
        Diagnostic::new(token.span, format!("expected {expected}, found {found}"))
    }

    fn is_symbol(&self, symbol: Symbol) -> bool {
        self.peek().kind == TokenKind::Symbol(symbol)
    }

    fn eat_symbol(&mut self, symbol: Symbol) -> bool {
        let found = self.is_symbol(symbol);
        if found {
            self.next();
        }
        found
    }

    fn expect_symbol(&mut self, symbol: Symbol) -> Result<Span> {
        match self.is_symbol(symbol) {
            true => Ok(self.next().span),
            false => Err(self.unexpected(&format!("`{}`", symbol.text()))),
        }
    }

    fn keyword_at(&self, offset: usize) -> Option<&str> {
        match &self.peek_at(offset).kind {
            TokenKind::Identifier(text) => Some(text),
            _ => None,
        }
    }

    fn is_keyword(&self, keyword: &str) -> bool {
        self.keyword_at(0) == Some(keyword)
    }

    fn eat_keyword(&mut self, keyword: &str) -> Option<Span> {
        self.is_keyword(keyword).then(|| self.next().span)
    }

    fn expect_keyword(&mut self, keyword: &str) -> Result<Span> {
        self.eat_keyword(keyword)
            .ok_or_else(|| self.unexpected(&format!("`{keyword}`")))
    }

    fn name(&mut self, what: &str) -> Result<Name> {
        match self.peek().kind.clone() {
            TokenKind::Identifier(text) => Ok(Name {
                text,
                span: self.next().span,
            }),
            _ => Err(self.unexpected(what)),
        }
    }

    fn docs(&mut self) -> Vec<DocLine> {
        let mut lines = Vec::new();
        while let TokenKind::Doc(text) = &self.peek().kind {
            let line = DocLine {
                text: text.clone(),
                span: self.peek().span,
            };
            lines.push(line);
            self.next();
        }
        lines
    }

    fn no_docs(&mut self, docs: &[DocLine], what: &str) -> Result<()> {
        match docs.first() {
            Some(line) => Err(Diagnostic::new(
                line.span,
                format!("a doc comment cannot document {what}"),
            )),
            None => Ok(()),
        }
    }

    fn file(&mut self) -> Result<File> {
        let docs = self.docs();
        self.expect_keyword("library")?;
        let name = self.name("the library name")?;
        self.expect_keyword("version")?;
        let version = self.literal()?;
        self.expect_keyword("abi_prefix")?;
        let abi_prefix = self.name("the ABI prefix")?;
        self.expect_symbol(Symbol::Semicolon)?;
        let mut items = Vec::new();
        loop {
            let docs = self.docs();
            if self.peek().kind == TokenKind::End {
                self.no_docs(&docs, "the end of the file")?;
                break;
            }
            let attributes = self.attributes()?;
            let kind = self.item_kind()?;
            items.push(Item {
                docs,
                attributes,
                kind,
            });
        }
        Ok(File {
            docs,
            name,
            version,
            abi_prefix,
            items,
        })
    }

    fn attributes(&mut self) -> Result<Vec<Attribute>> {
        let mut attributes = Vec::new();
        while self.eat_symbol(Symbol::At) {
            let name = self.name("an attribute name")?;
            let mut arguments = Vec::new();
            if self.eat_symbol(Symbol::OpenParen) {
                while !self.eat_symbol(Symbol::CloseParen) {
                    let key = match (&self.peek().kind, &self.peek_at(1).kind) {
                        (TokenKind::Identifier(_), TokenKind::Symbol(Symbol::Equals)) => {
                            let key = self.name("an argument name")?;
                            self.next();
                            Some(key)
                        }
                        _ => None,
                    };
                    let value = self.literal()?;
                    arguments.push(Argument { key, value });
                    if !self.is_symbol(Symbol::CloseParen) {
                        self.expect_symbol(Symbol::Comma)?;
                    }
                }
            }
            attributes.push(Attribute { name, arguments });
        }
        Ok(attributes)
    }

    fn literal(&mut self) -> Result<Literal> {
        let negative = self.eat_symbol(Symbol::Minus).then_some(());
        let token = self.peek().clone();
        let kind = match (token.kind, negative) {
            (TokenKind::Integer(value), None) => LiteralKind::Integer(value),
            (TokenKind::Integer(value), Some(())) => LiteralKind::Integer(-value),
            (TokenKind::Float(value), None) => LiteralKind::Float(value),
            (TokenKind::Float(value), Some(())) => LiteralKind::Float(-value),
            (TokenKind::String(text), None) => LiteralKind::String(text),
            (TokenKind::Identifier(text), None) => LiteralKind::Name(text),
            _ => return Err(self.unexpected("a value")),
        };
        self.next();
        Ok(Literal {
            kind,
            span: token.span,
        })
    }

    fn item_kind(&mut self) -> Result<ItemKind> {
        let keyword = self.keyword_at(0).map(str::to_owned);
        if let Some(lifetime) = keyword.as_deref().and_then(Lifetime::named) {
            self.next();
            return self.class(lifetime).map(ItemKind::Class);
        }
        match keyword.as_deref() {
            Some("const") => self.constant().map(ItemKind::Constant),
            Some("struct") => self.structure().map(ItemKind::Struct),
            Some("enum") => self.enumeration("enum").map(ItemKind::Enum),
            Some("flags") => self.enumeration("flags").map(ItemKind::Flags),
            Some("callback") => self.callback().map(ItemKind::Callback),
            Some("fn") => self.function().map(ItemKind::Function),
            _ => Err(self.unexpected(
                "`const`, `struct`, `enum`, `flags`, `callback`, `fn` or a class lifetime",
            )),
        }
    }

    fn constant(&mut self) -> Result<Constant> {
        self.expect_keyword("const")?;
        let name = self.name("a constant name")?;
        self.expect_symbol(Symbol::Colon)?;
        let ty = self.name("a type")?;
        self.expect_symbol(Symbol::Equals)?;
        let value = self.literal()?;
        self.expect_symbol(Symbol::Semicolon)?;
        Ok(Constant { name, ty, value })
    }

    fn structure(&mut self) -> Result<Struct> {
        self.expect_keyword("struct")?;
        let name = self.name("a struct name")?;
        self.expect_symbol(Symbol::OpenBrace)?;
        let mut fields = Vec::new();
        loop {
            let docs = self.docs();
            if self.eat_symbol(Symbol::CloseBrace) {
                self.no_docs(&docs, "a closing brace")?;
                break;
            }
            let field_name = self.name("a field name")?;
            self.expect_symbol(Symbol::Colon)?;
            let ty = self.name("a type")?;
            fields.push(Field {
                docs,
                name: field_name,
                ty,
            });
            if !self.is_symbol(Symbol::CloseBrace) {
                self.expect_symbol(Symbol::Comma)?;
            }
        }
        Ok(Struct { name, fields })
    }

    fn enumeration(&mut self, keyword: &str) -> Result<Enum> {
        self.expect_keyword(keyword)?;
        let name = self.name("a name")?;
        self.expect_symbol(Symbol::OpenBrace)?;
        let mut variants = Vec::new();
        loop {
            let docs = self.docs();
            if self.eat_symbol(Symbol::CloseBrace) {
                self.no_docs(&docs, "a closing brace")?;
                break;
            }
            let variant = self.name("a value name")?;
            let value = match self.eat_symbol(Symbol::Equals) {
                true => Some(self.literal()?),
                false => None,
            };
            variants.push(Variant {
                docs,
                name: variant,
                value,
            });
            if !self.is_symbol(Symbol::CloseBrace) {
                self.expect_symbol(Symbol::Comma)?;
            }
        }
        Ok(Enum { name, variants })
    }

    fn callback(&mut self) -> Result<Callback> {
        self.expect_keyword("callback")?;
        let name = self.name("a callback name")?;
        let (consumes, params) = self.params()?;
        if let Some(span) = consumes {
            return Err(Diagnostic::new(span, "a callback has no receiver"));
        }
        self.expect_symbol(Symbol::Semicolon)?;
        Ok(Callback { name, params })
    }

    fn function(&mut self) -> Result<Function> {
        self.expect_keyword("fn")?;
        let name = self.name("a function name")?;
        let (consumes, params) = self.params()?;
        let returns = match self.eat_symbol(Symbol::Arrow) {
            true => Some(self.type_expr()?),
            false => None,
        };
        let throws = match self.eat_keyword("throws") {
            Some(_) => Some(self.name("an error enum")?),
            None => None,
        };
        self.expect_symbol(Symbol::Semicolon)?;
        Ok(Function {
            name,
            consumes,
            params,
            returns,
            throws,
        })
    }

    fn params(&mut self) -> Result<(Option<Span>, Vec<Param>)> {
        self.expect_symbol(Symbol::OpenParen)?;
        let mut consumes = None;
        let mut params = Vec::new();
        while !self.eat_symbol(Symbol::CloseParen) {
            let docs = self.docs();
            self.no_docs(&docs, "a parameter; use `@param` in the doc comment above")?;
            let attributes = self.attributes()?;
            if params.is_empty()
                && consumes.is_none()
                && attributes.is_empty()
                && self.is_keyword("owned")
                && self.keyword_at(1) == Some("self")
            {
                let start = self.next().span;
                let end = self.next().span;
                consumes = Some(Span::new(
                    start.line,
                    start.column,
                    end.column + end.length - start.column,
                ));
            } else {
                let name = self.name("a parameter name")?;
                self.expect_symbol(Symbol::Colon)?;
                let ty = self.type_expr()?;
                params.push(Param {
                    attributes,
                    name,
                    ty,
                });
            }
            if !self.is_symbol(Symbol::CloseParen) {
                self.expect_symbol(Symbol::Comma)?;
            }
        }
        Ok((consumes, params))
    }

    fn type_expr(&mut self) -> Result<TypeExpr> {
        if let Some(span) = self.eat_keyword("owned") {
            return Ok(TypeExpr::Owned(Box::new(self.type_expr()?), span));
        }
        if let Some(span) = self.eat_keyword("borrowed") {
            return Ok(TypeExpr::Borrowed(Box::new(self.type_expr()?), span));
        }
        if self.is_symbol(Symbol::OpenBracket) {
            let span = self.next().span;
            let element = self.type_expr()?;
            self.expect_symbol(Symbol::CloseBracket)?;
            return Ok(TypeExpr::Array(Box::new(element), span));
        }
        if self.is_symbol(Symbol::OpenParen) {
            let span = self.next().span;
            let mut parts = Vec::new();
            while !self.eat_symbol(Symbol::CloseParen) {
                let name = self.name("a closure function name")?;
                self.expect_symbol(Symbol::Colon)?;
                let callback = self.name("a callback type")?;
                parts.push(ClosurePart { name, callback });
                if !self.is_symbol(Symbol::CloseParen) {
                    self.expect_symbol(Symbol::Comma)?;
                }
            }
            return Ok(TypeExpr::Closure(parts, span));
        }
        self.name("a type").map(TypeExpr::Named)
    }

    fn class(&mut self, lifetime: Lifetime) -> Result<Class> {
        self.expect_keyword("class")?;
        let name = self.name("a class name")?;
        self.expect_symbol(Symbol::OpenBrace)?;
        let mut members = Vec::new();
        loop {
            let docs = self.docs();
            if self.eat_symbol(Symbol::CloseBrace) {
                self.no_docs(&docs, "a closing brace")?;
                break;
            }
            let attributes = self.attributes()?;
            let kind = self.member_kind()?;
            members.push(Member {
                docs,
                attributes,
                kind,
            });
        }
        Ok(Class {
            lifetime,
            name,
            members,
        })
    }

    fn member_kind(&mut self) -> Result<MemberKind> {
        let keyword = self.keyword_at(0).map(str::to_owned);
        if let Some(kind) = keyword.as_deref().and_then(DeclarationKind::named) {
            self.next();
            let name = self.name("a declaration name")?;
            let (consumes, params) = self.params()?;
            if let Some(span) = consumes {
                return Err(Diagnostic::new(span, "a declaration has no receiver"));
            }
            self.expect_symbol(Symbol::Semicolon)?;
            return Ok(MemberKind::Declaration { kind, name, params });
        }
        match keyword.as_deref() {
            Some("constructor") => {
                let keyword = self.name("`constructor`")?;
                let name = match self.is_symbol(Symbol::OpenParen) {
                    true => None,
                    false => Some(self.name("a constructor name or `(`")?),
                };
                let (consumes, params) = self.params()?;
                if let Some(span) = consumes {
                    return Err(Diagnostic::new(span, "a constructor has no receiver"));
                }
                self.expect_symbol(Symbol::Semicolon)?;
                Ok(MemberKind::Constructor {
                    keyword,
                    name,
                    params,
                })
            }
            Some("fn") => self.function().map(MemberKind::Function),
            Some("property" | "readonly") => {
                let readonly = self.eat_keyword("readonly").is_some();
                self.expect_keyword("property")?;
                let name = self.name("a property name")?;
                self.expect_symbol(Symbol::Colon)?;
                let ty = self.type_expr()?;
                self.expect_symbol(Symbol::Semicolon)?;
                Ok(MemberKind::Property { name, ty, readonly })
            }
            _ => Err(self.unexpected(
                "`constructor`, `fn`, `property`, `readonly`, `container`, `element` or `modifier`",
            )),
        }
    }
}
