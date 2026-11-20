use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SieveSyntaxError {
    line: usize,
    column: usize,
    reason: &'static str,
}

impl SieveSyntaxError {
    #[must_use]
    pub fn line(&self) -> usize {
        self.line
    }

    #[must_use]
    pub fn column(&self) -> usize {
        self.column
    }

    #[must_use]
    pub fn reason(&self) -> &'static str {
        self.reason
    }
}

impl fmt::Display for SieveSyntaxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "line {}, column {}: {}",
            self.line, self.column, self.reason
        )
    }
}

impl std::error::Error for SieveSyntaxError {}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SieveArgument {
    Tag(String),
    Number(u64),
    String(String),
    StringList(Vec<String>),
}

impl SieveArgument {
    #[must_use]
    pub fn tag(&self) -> Option<&str> {
        match self {
            SieveArgument::Tag(tag) => Some(tag),
            _ => None,
        }
    }

    #[must_use]
    pub fn strings(&self) -> Option<Vec<&str>> {
        match self {
            SieveArgument::String(single) => Some(vec![single.as_str()]),
            SieveArgument::StringList(list) => Some(list.iter().map(String::as_str).collect()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SieveTest {
    name: String,
    arguments: Vec<SieveArgument>,
    tests: Vec<SieveTest>,
}

impl SieveTest {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn arguments(&self) -> &[SieveArgument] {
        &self.arguments
    }

    #[must_use]
    pub fn tests(&self) -> &[SieveTest] {
        &self.tests
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SieveCommand {
    name: String,
    arguments: Vec<SieveArgument>,
    tests: Vec<SieveTest>,
    block: Option<Vec<SieveCommand>>,
}

impl SieveCommand {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn arguments(&self) -> &[SieveArgument] {
        &self.arguments
    }

    #[must_use]
    pub fn tests(&self) -> &[SieveTest] {
        &self.tests
    }

    #[must_use]
    pub fn block(&self) -> Option<&[SieveCommand]> {
        self.block.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SieveScript {
    commands: Vec<SieveCommand>,
}

impl SieveScript {
    pub fn parse(source: &str) -> Result<SieveScript, SieveSyntaxError> {
        let tokens = Lexer::new(source).tokens()?;
        let mut parser = Parser {
            tokens,
            position: 0,
        };
        let commands = parser.commands(Scope::Script)?;
        Ok(SieveScript { commands })
    }

    #[must_use]
    pub fn commands(&self) -> &[SieveCommand] {
        &self.commands
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TokenKind {
    Identifier(String),
    Tag(String),
    Number(u64),
    Quoted(String),
    OpenBracket,
    CloseBracket,
    OpenParen,
    CloseParen,
    OpenBrace,
    CloseBrace,
    Comma,
    Semicolon,
}

#[derive(Debug, Clone)]
struct Token {
    kind: TokenKind,
    line: usize,
    column: usize,
}

struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    line: usize,
    column: usize,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Lexer<'a> {
        Lexer {
            chars: source.chars().peekable(),
            line: 1,
            column: 1,
        }
    }

    fn bump(&mut self) -> Option<char> {
        let character = self.chars.next()?;
        if character == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(character)
    }

    fn syntax_error(&self, reason: &'static str) -> SieveSyntaxError {
        SieveSyntaxError {
            line: self.line,
            column: self.column,
            reason,
        }
    }

    fn tokens(mut self) -> Result<Vec<Token>, SieveSyntaxError> {
        let mut tokens = Vec::new();

        while let Some(&character) = self.chars.peek() {
            let (line, column) = (self.line, self.column);
            let kind = match character {
                ' ' | '\t' | '\r' | '\n' => {
                    self.bump();
                    continue;
                }
                '#' => {
                    while self.bump().is_some_and(|skipped| skipped != '\n') {}
                    continue;
                }
                '/' => {
                    self.bump();
                    if self.bump() != Some('*') {
                        return Err(self.syntax_error("expected '*' after '/'"));
                    }
                    self.block_comment()?;
                    continue;
                }
                '"' => {
                    self.bump();
                    TokenKind::Quoted(self.quoted()?)
                }
                '[' => self.single(TokenKind::OpenBracket),
                ']' => self.single(TokenKind::CloseBracket),
                '(' => self.single(TokenKind::OpenParen),
                ')' => self.single(TokenKind::CloseParen),
                '{' => self.single(TokenKind::OpenBrace),
                '}' => self.single(TokenKind::CloseBrace),
                ',' => self.single(TokenKind::Comma),
                ';' => self.single(TokenKind::Semicolon),
                ':' => {
                    self.bump();
                    let name = self.word();
                    if name.is_empty() {
                        return Err(self.syntax_error("missing tag name"));
                    }
                    TokenKind::Tag(name.to_ascii_lowercase())
                }
                '0'..='9' => TokenKind::Number(self.number()?),
                character if character.is_ascii_alphabetic() || character == '_' => {
                    let word = self.word();
                    if word.eq_ignore_ascii_case("text") && self.chars.peek() == Some(&':') {
                        self.bump();
                        TokenKind::Quoted(self.multiline()?)
                    } else {
                        TokenKind::Identifier(word.to_ascii_lowercase())
                    }
                }
                _ => return Err(self.syntax_error("unexpected character")),
            };
            tokens.push(Token { kind, line, column });
        }

        Ok(tokens)
    }

    fn single(&mut self, kind: TokenKind) -> TokenKind {
        self.bump();
        kind
    }

    fn word(&mut self) -> String {
        let mut word = String::new();
        while let Some(&character) = self.chars.peek() {
            if !(character.is_ascii_alphanumeric() || character == '_') {
                break;
            }
            word.push(character);
            self.bump();
        }
        word
    }

    fn number(&mut self) -> Result<u64, SieveSyntaxError> {
        let mut value: u64 = 0;
        while let Some(&character) = self.chars.peek() {
            let Some(digit) = character.to_digit(10) else {
                break;
            };
            value = value
                .checked_mul(10)
                .and_then(|value| value.checked_add(u64::from(digit)))
                .ok_or_else(|| self.syntax_error("number too large"))?;
            self.bump();
        }

        let scale = match self.chars.peek().map(char::to_ascii_uppercase) {
            Some('K') => 1 << 10,
            Some('M') => 1 << 20,
            Some('G') => 1 << 30,
            _ => 1,
        };
        if scale > 1 {
            self.bump();
        }
        value
            .checked_mul(scale)
            .ok_or_else(|| self.syntax_error("number too large"))
    }

    fn quoted(&mut self) -> Result<String, SieveSyntaxError> {
        let mut text = String::new();
        loop {
            match self.bump() {
                Some('"') => return Ok(text),
                Some('\\') => match self.bump() {
                    Some(escaped) => text.push(escaped),
                    None => return Err(self.syntax_error("unterminated string")),
                },
                Some(character) => text.push(character),
                None => return Err(self.syntax_error("unterminated string")),
            }
        }
    }

    fn multiline(&mut self) -> Result<String, SieveSyntaxError> {
        loop {
            match self.bump() {
                Some(' ' | '\t' | '\r') => {}
                Some('#') => {
                    while self.bump().is_some_and(|skipped| skipped != '\n') {}
                    break;
                }
                Some('\n') => break,
                _ => return Err(self.syntax_error("expected line break after 'text:'")),
            }
        }

        let mut text = String::new();
        loop {
            let mut line = String::new();
            loop {
                match self.bump() {
                    Some('\n') => break,
                    Some(character) => line.push(character),
                    None => return Err(self.syntax_error("unterminated multi-line string")),
                }
            }
            let line = line.strip_suffix('\r').unwrap_or(&line);
            if line == "." {
                return Ok(text);
            }
            text.push_str(
                line.strip_prefix('.')
                    .filter(|_| line.starts_with(".."))
                    .unwrap_or(line),
            );
            text.push_str("\r\n");
        }
    }

    fn block_comment(&mut self) -> Result<(), SieveSyntaxError> {
        let mut previous = '\0';
        loop {
            match self.bump() {
                Some('/') if previous == '*' => return Ok(()),
                Some(character) => previous = character,
                None => return Err(self.syntax_error("unterminated comment")),
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    Script,
    Block,
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    fn peek(&self) -> Option<&TokenKind> {
        self.tokens.get(self.position).map(|token| &token.kind)
    }

    fn next(&mut self) -> Option<TokenKind> {
        let token = self.tokens.get(self.position)?.kind.clone();
        self.position += 1;
        Some(token)
    }

    fn syntax_error(&self, reason: &'static str) -> SieveSyntaxError {
        let (line, column) = self
            .tokens
            .get(self.position.saturating_sub(1))
            .map_or((1, 1), |token| (token.line, token.column));
        SieveSyntaxError {
            line,
            column,
            reason,
        }
    }

    fn commands(&mut self, scope: Scope) -> Result<Vec<SieveCommand>, SieveSyntaxError> {
        let mut commands = Vec::new();
        loop {
            match self.peek() {
                None if scope == Scope::Block => {
                    return Err(self.syntax_error("unterminated block"));
                }
                None => return Ok(commands),
                Some(TokenKind::CloseBrace) if scope == Scope::Block => {
                    self.position += 1;
                    return Ok(commands);
                }
                Some(TokenKind::Identifier(_)) => commands.push(self.command()?),
                Some(_) => {
                    self.position += 1;
                    return Err(self.syntax_error("expected command"));
                }
            }
        }
    }

    fn command(&mut self) -> Result<SieveCommand, SieveSyntaxError> {
        let Some(TokenKind::Identifier(name)) = self.next() else {
            return Err(self.syntax_error("expected command"));
        };
        let (arguments, tests) = self.arguments()?;

        let block = match self.next() {
            Some(TokenKind::Semicolon) => None,
            Some(TokenKind::OpenBrace) => Some(self.commands(Scope::Block)?),
            _ => return Err(self.syntax_error("expected ';' or block")),
        };

        Ok(SieveCommand {
            name,
            arguments,
            tests,
            block,
        })
    }

    fn arguments(&mut self) -> Result<(Vec<SieveArgument>, Vec<SieveTest>), SieveSyntaxError> {
        let mut arguments = Vec::new();
        loop {
            match self.peek() {
                Some(
                    TokenKind::Tag(_)
                    | TokenKind::Number(_)
                    | TokenKind::Quoted(_)
                    | TokenKind::OpenBracket,
                ) => {
                    arguments.push(self.argument()?);
                }
                Some(TokenKind::Identifier(_)) => return Ok((arguments, vec![self.test()?])),
                Some(TokenKind::OpenParen) => return Ok((arguments, self.test_list()?)),
                _ => return Ok((arguments, Vec::new())),
            }
        }
    }

    fn argument(&mut self) -> Result<SieveArgument, SieveSyntaxError> {
        match self.next() {
            Some(TokenKind::Tag(tag)) => Ok(SieveArgument::Tag(tag)),
            Some(TokenKind::Number(number)) => Ok(SieveArgument::Number(number)),
            Some(TokenKind::Quoted(text)) => Ok(SieveArgument::String(text)),
            Some(TokenKind::OpenBracket) => {
                let mut list = Vec::new();
                loop {
                    match self.next() {
                        Some(TokenKind::Quoted(text)) => list.push(text),
                        _ => return Err(self.syntax_error("expected string in string list")),
                    }
                    match self.next() {
                        Some(TokenKind::Comma) => {}
                        Some(TokenKind::CloseBracket) => {
                            return Ok(SieveArgument::StringList(list));
                        }
                        _ => return Err(self.syntax_error("unterminated string list")),
                    }
                }
            }
            _ => Err(self.syntax_error("expected argument")),
        }
    }

    fn test(&mut self) -> Result<SieveTest, SieveSyntaxError> {
        let Some(TokenKind::Identifier(name)) = self.next() else {
            return Err(self.syntax_error("expected test"));
        };
        let (arguments, tests) = self.arguments()?;
        Ok(SieveTest {
            name,
            arguments,
            tests,
        })
    }

    fn test_list(&mut self) -> Result<Vec<SieveTest>, SieveSyntaxError> {
        self.position += 1;
        let mut tests = Vec::new();
        loop {
            tests.push(self.test()?);
            match self.next() {
                Some(TokenKind::Comma) => {}
                Some(TokenKind::CloseParen) => return Ok(tests),
                _ => return Err(self.syntax_error("unterminated test list")),
            }
        }
    }
}
