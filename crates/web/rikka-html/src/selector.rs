use std::str::FromStr;

use crate::document::Element;
use crate::error::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selector {
    alternatives: Vec<Complex>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Complex {
    subject: Compound,
    ancestors: Vec<(Combinator, Compound)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Combinator {
    Descendant,
    Child,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Compound {
    name: Option<Box<str>>,
    id: Option<Box<str>>,
    classes: Vec<Box<str>>,
    attributes: Vec<Attribute>,
    excluded: Vec<Compound>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Attribute {
    name: Box<str>,
    test: Test,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Test {
    Present,
    Equals(Box<str>),
    Contains(Box<str>),
}

impl Selector {
    pub fn parse(css: &str) -> Result<Selector> {
        let mut parser = Parser {
            css,
            bytes: css.as_bytes(),
            at: 0,
        };
        let mut alternatives = vec![parser.complex()?];
        while parser.peek() == Some(b',') {
            parser.at += 1;
            alternatives.push(parser.complex()?);
        }
        match parser.peek() {
            None => Ok(Selector { alternatives }),
            Some(_) => Err(parser.fail("a combinator or `,`")),
        }
    }

    #[must_use]
    pub fn matches(&self, element: Element<'_>) -> bool {
        self.alternatives
            .iter()
            .any(|complex| complex.matches(element))
    }
}

impl FromStr for Selector {
    type Err = Error;

    fn from_str(css: &str) -> Result<Selector> {
        Selector::parse(css)
    }
}

impl Complex {
    fn matches(&self, element: Element<'_>) -> bool {
        self.subject.matches(element) && related(&self.ancestors, element)
    }
}

fn related(ancestors: &[(Combinator, Compound)], element: Element<'_>) -> bool {
    let Some(((combinator, compound), outer)) = ancestors.split_first() else {
        return true;
    };
    let mut candidate = element.parent();
    while let Some(ancestor) = candidate {
        if compound.matches(ancestor) && related(outer, ancestor) {
            return true;
        }
        if *combinator == Combinator::Child {
            return false;
        }
        candidate = ancestor.parent();
    }
    false
}

impl Compound {
    fn matches(&self, element: Element<'_>) -> bool {
        self.name
            .as_deref()
            .is_none_or(|name| element.name() == name)
            && self
                .id
                .as_deref()
                .is_none_or(|id| element.attribute("id") == Some(id))
            && self.classes.iter().all(|class| element.has_class(class))
            && self
                .attributes
                .iter()
                .all(|attribute| attribute.matches(element))
            && !self
                .excluded
                .iter()
                .any(|excluded| excluded.matches(element))
    }

    fn is_empty(&self) -> bool {
        *self == Compound::default()
    }
}

impl Attribute {
    fn matches(&self, element: Element<'_>) -> bool {
        let Some(value) = element.attribute(&self.name) else {
            return false;
        };
        match &self.test {
            Test::Present => true,
            Test::Equals(wanted) => value == wanted.as_ref(),
            Test::Contains(wanted) => !wanted.is_empty() && value.contains(wanted.as_ref()),
        }
    }
}

struct Parser<'a> {
    css: &'a str,
    bytes: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn complex(&mut self) -> Result<Complex> {
        let mut subject = self.compound()?;
        let mut ancestors = Vec::new();
        loop {
            let spaced = self.skip_spaces();
            let combinator = match self.bytes.get(self.at) {
                Some(b'>') => {
                    self.at += 1;
                    Combinator::Child
                }
                Some(b',') | None => break,
                Some(_) if spaced => Combinator::Descendant,
                Some(_) => return Err(self.fail("a combinator or `,`")),
            };
            let inner = self.compound()?;
            ancestors.insert(0, (combinator, std::mem::replace(&mut subject, inner)));
        }
        Ok(Complex { subject, ancestors })
    }

    fn compound(&mut self) -> Result<Compound> {
        self.skip_spaces();
        let mut compound = Compound::default();
        let mut universal = false;
        match self.bytes.get(self.at) {
            Some(b'*') => {
                self.at += 1;
                universal = true;
            }
            Some(byte) if names(*byte) => {
                compound.name = Some(self.identifier()?.to_ascii_lowercase().into());
            }
            _ => {}
        }
        loop {
            match self.bytes.get(self.at) {
                Some(b'#') => {
                    self.at += 1;
                    compound.id = Some(self.identifier()?.into());
                }
                Some(b'.') => {
                    self.at += 1;
                    compound.classes.push(self.identifier()?.into());
                }
                Some(b'[') => {
                    self.at += 1;
                    compound.attributes.push(self.attribute()?);
                }
                Some(b':') => {
                    self.at += 1;
                    compound.excluded.push(self.exclusion()?);
                }
                _ => break,
            }
        }
        match compound.is_empty() && !universal {
            true => Err(self.fail("a selector")),
            false => Ok(compound),
        }
    }

    fn attribute(&mut self) -> Result<Attribute> {
        self.skip_spaces();
        let name = self.identifier()?.to_ascii_lowercase().into();
        self.skip_spaces();
        let contains = match self.bytes.get(self.at..) {
            Some([b']', ..]) => {
                self.at += 1;
                return Ok(Attribute {
                    name,
                    test: Test::Present,
                });
            }
            Some([b'=', ..]) => {
                self.at += 1;
                false
            }
            Some([b'*', b'=', ..]) => {
                self.at += 2;
                true
            }
            _ => return Err(self.fail("`]`, `=` or `*=`")),
        };
        self.skip_spaces();
        let value = self.value()?.into();
        self.skip_spaces();
        self.expect(b']', "`]`")?;
        Ok(Attribute {
            name,
            test: match contains {
                true => Test::Contains(value),
                false => Test::Equals(value),
            },
        })
    }

    fn exclusion(&mut self) -> Result<Compound> {
        if self.identifier()? != "not" {
            return Err(self.fail("`not`"));
        }
        self.expect(b'(', "`(`")?;
        let excluded = self.compound()?;
        self.skip_spaces();
        self.expect(b')', "`)`")?;
        Ok(excluded)
    }

    fn value(&mut self) -> Result<&str> {
        let Some(quote @ (b'"' | b'\'')) = self.bytes.get(self.at).copied() else {
            return self.identifier();
        };
        let from = self.at + 1;
        let Some(length) = self.css[from..].find(char::from(quote)) else {
            return Err(self.fail("a closing quote"));
        };
        self.at = from + length + 1;
        Ok(&self.css[from..from + length])
    }

    fn identifier(&mut self) -> Result<&str> {
        let from = self.at;
        while self.bytes.get(self.at).copied().is_some_and(names) {
            self.at += 1;
        }
        match self.at == from {
            true => Err(self.fail("a name")),
            false => Ok(&self.css[from..self.at]),
        }
    }

    fn expect(&mut self, byte: u8, expected: &'static str) -> Result<()> {
        match self.bytes.get(self.at) == Some(&byte) {
            true => {
                self.at += 1;
                Ok(())
            }
            false => Err(self.fail(expected)),
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_spaces();
        self.bytes.get(self.at).copied()
    }

    fn skip_spaces(&mut self) -> bool {
        let from = self.at;
        while self.bytes.get(self.at).is_some_and(u8::is_ascii_whitespace) {
            self.at += 1;
        }
        self.at > from
    }

    fn fail(&self, expected: &'static str) -> Error {
        Error::Selector {
            selector: self.css.to_owned(),
            at: self.at,
            expected,
        }
    }
}

fn names(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_') || byte >= 0x80
}
