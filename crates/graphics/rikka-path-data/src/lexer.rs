use crate::error::{Error, ErrorKind};

#[derive(Clone, Debug)]
pub(crate) struct Lexer<'a> {
    text: &'a str,
    at: usize,
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(text: &'a str) -> Lexer<'a> {
        Lexer { text, at: 0 }
    }

    pub(crate) fn offset(&self) -> usize {
        self.at
    }

    pub(crate) fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.at).copied()
    }

    pub(crate) fn peek_char(&self) -> Option<char> {
        self.text[self.at..].chars().next()
    }

    pub(crate) fn advance(&mut self) {
        self.at += 1;
    }

    pub(crate) fn skip_spaces(&mut self) {
        while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
            self.at += 1;
        }
    }

    pub(crate) fn skip_separators(&mut self) {
        self.skip_spaces();
        if self.peek() == Some(b',') {
            self.at += 1;
            self.skip_spaces();
        }
    }

    pub(crate) fn starts_number(&self) -> bool {
        self.peek()
            .is_some_and(|byte| byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.'))
    }

    pub(crate) fn number(&mut self) -> Result<f32, Error> {
        self.skip_separators();
        let bytes = self.text.as_bytes();
        let start = self.at;
        let mut at = start;
        if matches!(bytes.get(at), Some(b'+' | b'-')) {
            at += 1;
        }
        let digits_from = at;
        while bytes.get(at).is_some_and(u8::is_ascii_digit) {
            at += 1;
        }
        let mut digits = at - digits_from;
        if bytes.get(at) == Some(&b'.') {
            at += 1;
            let fraction_from = at;
            while bytes.get(at).is_some_and(u8::is_ascii_digit) {
                at += 1;
            }
            digits += at - fraction_from;
        }
        if digits == 0 {
            return Err(self.fault(ErrorKind::Number));
        }
        if matches!(bytes.get(at), Some(b'e' | b'E')) {
            let mut exponent = at + 1;
            if matches!(bytes.get(exponent), Some(b'+' | b'-')) {
                exponent += 1;
            }
            if bytes.get(exponent).is_some_and(u8::is_ascii_digit) {
                while bytes.get(exponent).is_some_and(u8::is_ascii_digit) {
                    exponent += 1;
                }
                at = exponent;
            }
        }
        match self.text[start..at].parse::<f32>() {
            Ok(value) if value.is_finite() => {
                self.at = at;
                Ok(value)
            }
            _ => Err(self.fault(ErrorKind::Number)),
        }
    }

    pub(crate) fn flag(&mut self) -> Result<bool, Error> {
        self.skip_separators();
        let flag = match self.peek() {
            Some(b'0') => false,
            Some(b'1') => true,
            _ => return Err(self.fault(ErrorKind::Flag)),
        };
        self.at += 1;
        Ok(flag)
    }

    pub(crate) fn fault(&self, kind: ErrorKind) -> Error {
        let kind = match (kind, self.peek()) {
            (ErrorKind::Number | ErrorKind::Flag, None) => ErrorKind::UnexpectedEnd,
            (kind, _) => kind,
        };
        Error {
            offset: self.at,
            kind,
        }
    }
}
