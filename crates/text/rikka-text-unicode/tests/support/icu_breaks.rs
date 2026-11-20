#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Iteration {
    Word,
    Line,
    Other,
}

#[derive(Debug)]
pub(crate) struct IcuCase {
    pub(crate) line: usize,
    pub(crate) iteration: Iteration,
    pub(crate) text: String,
    pub(crate) breaks: Vec<usize>,
}

struct Reader<'a> {
    source: &'a str,
    at: usize,
    line: usize,
}

impl Reader<'_> {
    fn rest(&self) -> &str {
        &self.source[self.at..]
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn take(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.at += character.len_utf8();
        if character == '\n' {
            self.line += 1;
        }
        Some(character)
    }

    fn eat(&mut self, prefix: &str) -> bool {
        let found = self.rest().starts_with(prefix);
        if found {
            self.at += prefix.len();
        }
        found
    }

    fn skip_past(&mut self, needle: &str) {
        while !self.rest().is_empty() && !self.eat(needle) {
            self.take();
        }
    }

    fn column(&self) -> usize {
        self.at
            - self.source[..self.at]
                .rfind('\n')
                .map_or(0, |newline| newline + 1)
    }

    fn hex(&mut self, digits: usize) -> Option<char> {
        let value = u32::from_str_radix(self.rest().get(..digits)?, 16).ok()?;
        self.at += digits;
        char::from_u32(value)
    }

    fn escape(&mut self) -> Option<char> {
        if self.eat("u") {
            return self.hex(4);
        }
        if self.eat("U") {
            return self.hex(8);
        }
        if self.eat("x{") {
            let close = self.rest().find('}')?;
            let value = u32::from_str_radix(&self.rest()[..close], 16).ok()?;
            self.at += close + 1;
            return char::from_u32(value);
        }
        if self.eat("x") {
            return self.hex(2);
        }
        let literal = self.take()?;
        Some(match literal {
            't' => '\t',
            'n' => '\n',
            'r' => '\r',
            'f' => '\u{C}',
            'v' => '\u{B}',
            'a' => '\u{7}',
            'e' => '\u{1B}',
            other => other,
        })
    }

    fn data(&mut self) -> Option<(String, Vec<usize>)> {
        let mut text = String::new();
        let mut breaks = Vec::new();
        let mut supported = true;
        loop {
            if self.eat("</data>") {
                return supported.then_some((text, breaks));
            }
            if self.eat("<>") {
                breaks.push(text.len());
                continue;
            }
            if self.eat("\\N{") {
                self.skip_past("}");
                supported = false;
                continue;
            }
            let column = self.column();
            let character = self.take()?;
            match character {
                '•' => breaks.push(text.len()),
                '<' => {
                    self.skip_past(">");
                    breaks.push(text.len());
                }
                '#' if column == 1 => self.skip_past("\n"),
                '\\' if self.eat("\r\n") || self.eat("\n") => {}
                '\\' => text.push(self.escape()?),
                other => text.push(other),
            }
        }
    }
}

pub(crate) fn icu_cases(source: &str) -> Vec<IcuCase> {
    let mut reader = Reader {
        source: source.trim_start_matches('\u{FEFF}'),
        at: 0,
        line: 1,
    };
    let mut iteration = Iteration::Other;
    let mut cases = Vec::new();
    while let Some(character) = reader.peek() {
        let line = reader.line;
        if character == '#' {
            reader.skip_past("\n");
        } else if character.is_whitespace() {
            reader.take();
        } else if reader.eat("<word>") {
            iteration = Iteration::Word;
        } else if reader.eat("<line>") {
            iteration = Iteration::Line;
        } else if reader.eat("<rules>") {
            reader.skip_past("</rules>");
            iteration = Iteration::Other;
        } else if reader.eat("<badrules>") {
            reader.skip_past("</badrules>");
        } else if reader.eat("<data>") {
            if let Some((text, breaks)) = reader.data() {
                cases.push(IcuCase {
                    line,
                    iteration,
                    text,
                    breaks: breaks.into_iter().filter(|&offset| offset > 0).collect(),
                });
            }
        } else if reader.eat("<") {
            let tag = reader
                .rest()
                .split('>')
                .next()
                .unwrap_or_default()
                .to_owned();
            reader.skip_past(">");
            if !tag.starts_with("locale") {
                iteration = Iteration::Other;
            }
        } else {
            reader.take();
        }
    }
    cases
}
