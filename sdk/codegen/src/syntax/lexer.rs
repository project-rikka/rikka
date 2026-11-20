use crate::diagnostic::{Diagnostic, Result, Span};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TokenKind {
    Identifier(String),
    Integer(i128),
    Float(f64),
    String(String),
    Doc(String),
    Symbol(Symbol),
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Symbol {
    OpenBrace,
    CloseBrace,
    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    Comma,
    Semicolon,
    Colon,
    Equals,
    At,
    Minus,
    Arrow,
}

impl Symbol {
    pub(crate) fn text(self) -> &'static str {
        match self {
            Self::OpenBrace => "{",
            Self::CloseBrace => "}",
            Self::OpenParen => "(",
            Self::CloseParen => ")",
            Self::OpenBracket => "[",
            Self::CloseBracket => "]",
            Self::Comma => ",",
            Self::Semicolon => ";",
            Self::Colon => ":",
            Self::Equals => "=",
            Self::At => "@",
            Self::Minus => "-",
            Self::Arrow => "->",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) span: Span,
}

pub(crate) fn tokenize(source: &str) -> Result<Vec<Token>> {
    let mut lexer = Lexer {
        characters: source.chars().collect(),
        position: 0,
        line: 1,
        column: 1,
        tokens: Vec::new(),
    };
    lexer.run()?;
    Ok(lexer.tokens)
}

struct Lexer {
    characters: Vec<char>,
    position: usize,
    line: usize,
    column: usize,
    tokens: Vec<Token>,
}

impl Lexer {
    fn peek(&self, offset: usize) -> Option<char> {
        self.characters.get(self.position + offset).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let character = self.peek(0)?;
        self.position += 1;
        match character {
            '\n' => {
                self.line += 1;
                self.column = 1;
            }
            _ => self.column += 1,
        }
        Some(character)
    }

    fn take_while(&mut self, accept: impl Fn(char) -> bool) -> String {
        let mut taken = String::new();
        while let Some(character) = self.peek(0).filter(|character| accept(*character)) {
            taken.push(character);
            self.advance();
        }
        taken
    }

    fn push(&mut self, kind: TokenKind, line: usize, column: usize) {
        let length = match self.line == line {
            true => self.column - column,
            false => 1,
        };
        self.tokens.push(Token {
            kind,
            span: Span::new(line, column, length),
        });
    }

    fn run(&mut self) -> Result<()> {
        while let Some(character) = self.peek(0) {
            let (line, column) = (self.line, self.column);
            match character {
                _ if character.is_whitespace() => {
                    self.advance();
                }
                '/' if self.peek(1) == Some('/') => self.comment(line, column),
                '"' => self.string(line, column)?,
                '0'..='9' => self.number(line, column)?,
                _ if character.is_ascii_alphabetic() || character == '_' => {
                    let text = self.take_while(|next| next.is_ascii_alphanumeric() || next == '_');
                    self.push(TokenKind::Identifier(text), line, column);
                }
                _ => self.symbol(character, line, column)?,
            }
        }
        let (line, column) = (self.line, self.column);
        self.push(TokenKind::End, line, column);
        Ok(())
    }

    fn comment(&mut self, line: usize, column: usize) {
        self.advance();
        self.advance();
        let doc = self.peek(0) == Some('/') && self.peek(1) != Some('/');
        let raw = self.take_while(|next| next != '\n');
        if doc {
            let body = &raw[1..];
            let (text, skipped) = match body.strip_prefix(' ') {
                Some(text) => (text.trim_end(), 4),
                None => (body.trim_end(), 3),
            };
            self.tokens.push(Token {
                kind: TokenKind::Doc(text.to_owned()),
                span: Span::new(line, column + skipped, text.chars().count()),
            });
        }
    }

    fn string(&mut self, line: usize, column: usize) -> Result<()> {
        self.advance();
        let mut text = String::new();
        loop {
            match self.advance() {
                Some('"') => break,
                Some('\\') => match self.advance() {
                    Some(escaped @ ('"' | '\\')) => text.push(escaped),
                    _ => {
                        return Err(Diagnostic::new(
                            Span::new(self.line, self.column - 1, 1),
                            "unknown escape in string; only \\\" and \\\\ are allowed",
                        ));
                    }
                },
                Some('\n') | None => {
                    return Err(Diagnostic::new(
                        Span::new(line, column, 1),
                        "string is not closed on its line",
                    ));
                }
                Some(character) => text.push(character),
            }
        }
        self.push(TokenKind::String(text), line, column);
        Ok(())
    }

    fn number(&mut self, line: usize, column: usize) -> Result<()> {
        let invalid = |lexer: &Self| {
            Diagnostic::new(
                Span::new(line, column, lexer.column - column),
                "invalid number",
            )
        };
        let kind = if self.peek(0) == Some('0') && matches!(self.peek(1), Some('x' | 'X')) {
            self.advance();
            self.advance();
            let digits = self.take_while(|next| next.is_ascii_hexdigit());
            TokenKind::Integer(i128::from_str_radix(&digits, 16).map_err(|_| invalid(self))?)
        } else {
            let mut text = self.take_while(|next| next.is_ascii_digit());
            if self.peek(0) == Some('.') && self.peek(1).is_some_and(|next| next.is_ascii_digit()) {
                self.advance();
                text.push('.');
                text.push_str(&self.take_while(|next| next.is_ascii_digit()));
                TokenKind::Float(text.parse().map_err(|_| invalid(self))?)
            } else {
                TokenKind::Integer(text.parse().map_err(|_| invalid(self))?)
            }
        };
        if self
            .peek(0)
            .is_some_and(|next| next.is_ascii_alphanumeric() || next == '_')
        {
            return Err(invalid(self));
        }
        self.push(kind, line, column);
        Ok(())
    }

    fn symbol(&mut self, character: char, line: usize, column: usize) -> Result<()> {
        let symbol = match character {
            '{' => Symbol::OpenBrace,
            '}' => Symbol::CloseBrace,
            '(' => Symbol::OpenParen,
            ')' => Symbol::CloseParen,
            '[' => Symbol::OpenBracket,
            ']' => Symbol::CloseBracket,
            ',' => Symbol::Comma,
            ';' => Symbol::Semicolon,
            ':' => Symbol::Colon,
            '=' => Symbol::Equals,
            '@' => Symbol::At,
            '-' if self.peek(1) == Some('>') => {
                self.advance();
                Symbol::Arrow
            }
            '-' => Symbol::Minus,
            _ => {
                return Err(Diagnostic::new(
                    Span::new(line, column, 1),
                    format!("unexpected character `{character}`"),
                ));
            }
        };
        self.advance();
        self.push(TokenKind::Symbol(symbol), line, column);
        Ok(())
    }
}
