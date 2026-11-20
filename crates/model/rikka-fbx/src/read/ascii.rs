use crate::document::{Node, Value};
use rikka_model_core::ImportCtx;
use rikka_model_core::{Error, Result};

const MAX_DEPTH: usize = 64;
const DEFAULT_VERSION: u32 = 7400;

pub(crate) fn parse(bytes: &[u8], ctx: &mut ImportCtx<'_>) -> Result<(u32, Vec<Node>)> {
    let mut scanner = Scanner::new(bytes);
    let mut nodes = Vec::new();
    loop {
        ctx.checkpoint()?;
        match scanner.peek()? {
            Token::End => break,
            _ => nodes.push(scanner.node(0, ctx)?),
        }
    }
    Ok((version(&nodes), nodes))
}

fn version(nodes: &[Node]) -> u32 {
    nodes
        .iter()
        .find(|node| node.name == "FBXHeaderExtension")
        .and_then(|header| header.child_integer("FBXVersion"))
        .and_then(|version| u32::try_from(version).ok())
        .unwrap_or(DEFAULT_VERSION)
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    End,
    Name(String),
    Text(String),
    Word(String),
    Integer(i64),
    Number(f64),
    Symbol(u8),
}

struct Scanner<'a> {
    bytes: &'a [u8],
    at: usize,
    peeked: Option<Token>,
}

impl<'a> Scanner<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            at: 0,
            peeked: None,
        }
    }

    fn node(&mut self, depth: usize, ctx: &mut ImportCtx<'_>) -> Result<Node> {
        if depth > MAX_DEPTH {
            return Err(Error::Damaged(format!(
                "records nest deeper than {MAX_DEPTH}"
            )));
        }
        let Token::Name(name) = self.next()? else {
            return Err(self.damaged("record must start with a name"));
        };
        let mut node = Node::new(name);

        loop {
            match self.peek()? {
                Token::Text(_) | Token::Word(_) | Token::Integer(_) | Token::Number(_) => {
                    node.values.push(self.value()?);
                }
                Token::Symbol(b'*') => {
                    self.next()?;
                    self.array(&mut node, ctx)?;
                }
                _ => break,
            }
            match self.peek()? == Token::Symbol(b',') {
                true => {
                    self.next()?;
                }
                false => break,
            }
        }

        if self.peek()? == Token::Symbol(b'{') {
            self.next()?;
            while self.peek()? != Token::Symbol(b'}') {
                if self.peek()? == Token::End {
                    return Err(self.damaged("unterminated record"));
                }
                node.children.push(self.node(depth + 1, ctx)?);
            }
            self.next()?;
        }
        Ok(node)
    }

    fn array(&mut self, node: &mut Node, ctx: &mut ImportCtx<'_>) -> Result<()> {
        let Token::Integer(count) = self.next()? else {
            return Err(self.damaged("array is missing a length"));
        };
        if self.peek()? != Token::Symbol(b'{') {
            node.values.push(Value::I64(count));
            return Ok(());
        }
        self.next()?;
        let Token::Name(_) = self.next()? else {
            return Err(self.damaged("array values must be named"));
        };
        let mut values = Vec::with_capacity(count.clamp(0, 4096) as usize);
        loop {
            match self.peek()? {
                Token::Symbol(b'}') | Token::End => break,
                Token::Symbol(b',') => {
                    self.next()?;
                }
                _ => {
                    values.push(self.value()?);
                    ctx.charge(size_of::<Value>() as u64)?;
                }
            }
        }
        if self.next()? != Token::Symbol(b'}') {
            return Err(self.damaged("unterminated array"));
        }
        node.values.push(pack(values));
        Ok(())
    }

    fn value(&mut self) -> Result<Value> {
        Ok(match self.next()? {
            Token::Text(text) => Value::Text(text),
            Token::Word(word) => Value::Word(word),
            Token::Integer(value) => Value::I64(value),
            Token::Number(value) => Value::F64(value),
            _ => return Err(self.damaged("expected a value")),
        })
    }

    fn peek(&mut self) -> Result<Token> {
        if self.peeked.is_none() {
            self.peeked = Some(self.scan()?);
        }
        Ok(self
            .peeked
            .clone()
            .expect("tokens are buffered before being taken"))
    }

    fn next(&mut self) -> Result<Token> {
        match self.peeked.take() {
            Some(token) => Ok(token),
            None => self.scan(),
        }
    }

    fn scan(&mut self) -> Result<Token> {
        self.skip_blank();
        let Some(&byte) = self.bytes.get(self.at) else {
            return Ok(Token::End);
        };
        match byte {
            b'"' => {
                self.at += 1;
                let text = self.quoted();
                Ok(self.named(text, Token::Text))
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' => {
                let word = self.bare();
                Ok(self.named(word, Token::Word))
            }
            byte if byte.is_ascii_digit() || matches!(byte, b'-' | b'+' | b'.') => self.number(),
            byte => {
                self.at += 1;
                Ok(Token::Symbol(byte))
            }
        }
    }

    fn named(&mut self, text: String, value: fn(String) -> Token) -> Token {
        let start = self.at;
        self.skip_blank();
        if self.bytes.get(self.at) == Some(&b':') {
            self.at += 1;
            return Token::Name(text);
        }
        self.at = start;
        value(text)
    }

    fn quoted(&mut self) -> String {
        let mut text = String::new();
        while let Some(&byte) = self.bytes.get(self.at) {
            self.at += 1;
            match byte {
                b'"' => break,
                b'&' => match self.entity() {
                    Some(replacement) => text.push(replacement),
                    None => text.push('&'),
                },
                byte => text.push(char::from(byte)),
            }
        }
        text
    }

    fn entity(&mut self) -> Option<char> {
        for (name, replacement) in [("quot;", '"'), ("cr;", '\r'), ("lf;", '\n')] {
            if self.bytes[self.at..].starts_with(name.as_bytes()) {
                self.at += name.len();
                return Some(replacement);
            }
        }
        None
    }

    fn bare(&mut self) -> String {
        let start = self.at;
        while self
            .bytes
            .get(self.at)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            self.at += 1;
        }
        String::from_utf8_lossy(&self.bytes[start..self.at]).into_owned()
    }

    fn number(&mut self) -> Result<Token> {
        let start = self.at;
        let mut real = false;
        while let Some(&byte) = self.bytes.get(self.at) {
            match byte {
                b'.' | b'#' => real = true,
                b'e' | b'E' if self.at > start => real = true,
                byte if byte.is_ascii_digit() => {}
                b'-' | b'+' => {}
                byte if byte.is_ascii_alphabetic() && real => {}
                _ => break,
            }
            self.at += 1;
        }
        let text = String::from_utf8_lossy(&self.bytes[start..self.at]);
        if !real && let Ok(value) = text.parse::<i64>() {
            return Ok(Token::Integer(value));
        }
        match text.parse::<f64>() {
            Ok(value) => Ok(Token::Number(value)),
            Err(_) => Ok(Token::Number(unrepresentable(&text))),
        }
    }

    fn skip_blank(&mut self) {
        while let Some(&byte) = self.bytes.get(self.at) {
            match byte {
                b';' => {
                    while !matches!(self.bytes.get(self.at), None | Some(b'\n')) {
                        self.at += 1;
                    }
                }
                byte if byte.is_ascii_whitespace() => self.at += 1,
                _ => break,
            }
        }
    }

    fn damaged(&self, what: &str) -> Error {
        let line = 1 + self.bytes[..self.at.min(self.bytes.len())]
            .iter()
            .filter(|&&byte| byte == b'\n')
            .count();
        Error::Damaged(format!("{what} on line {line}"))
    }
}

fn unrepresentable(text: &str) -> f64 {
    let negative = text.starts_with('-');
    let magnitude = match text.contains("INF") || text.contains("inf") {
        true => f64::INFINITY,
        false => f64::NAN,
    };
    match negative {
        true => -magnitude,
        false => magnitude,
    }
}

fn pack(values: Vec<Value>) -> Value {
    let real = values
        .iter()
        .any(|value| matches!(value, Value::F64(_) | Value::F32(_)));
    match real {
        true => Value::F64s(
            values
                .iter()
                .map(|value| value.as_number().unwrap_or(0.0))
                .collect(),
        ),
        false => Value::I64s(
            values
                .iter()
                .map(|value| value.as_integer().unwrap_or(0))
                .collect(),
        ),
    }
}
