use crate::pattern::{Part, Pattern, PluralArm, SelectArm, Selector};
use crate::plural::{PluralCategory, PluralKind};

#[derive(Debug, thiserror::Error)]
#[error("{reason} at byte {offset}")]
pub struct SyntaxError {
    reason: String,
    offset: usize,
}

pub(crate) fn parse(source: &str) -> Result<Pattern, SyntaxError> {
    Parser { source, at: 0 }.pattern(Scope::TOP)
}

#[derive(Clone, Copy)]
struct Scope {
    arm: bool,
    plural: bool,
}

impl Scope {
    const TOP: Self = Self {
        arm: false,
        plural: false,
    };

    fn arm(self, plural: bool) -> Self {
        Self {
            arm: true,
            plural: self.plural || plural,
        }
    }
}

struct Parser<'a> {
    source: &'a str,
    at: usize,
}

impl<'a> Parser<'a> {
    fn error(&self, reason: impl Into<String>) -> SyntaxError {
        SyntaxError {
            reason: reason.into(),
            offset: self.at,
        }
    }

    fn rest(&self) -> &'a str {
        &self.source[self.at..]
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn eat(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.at += expected.len_utf8();
            true
        } else {
            false
        }
    }

    fn skip_whitespace(&mut self) {
        self.at += self.rest().len() - self.rest().trim_start().len();
    }

    fn pattern(&mut self, scope: Scope) -> Result<Pattern, SyntaxError> {
        let mut parts = Vec::new();
        let mut literal = String::new();

        while let Some(next) = self.peek() {
            match next {
                '\'' => {
                    self.at += 1;
                    self.quoted(&mut literal);
                }
                '{' => {
                    self.at += 1;
                    push_literal(&mut parts, &mut literal);
                    parts.push(self.placeholder(scope)?);
                }
                '}' => {
                    if scope.arm {
                        break;
                    }
                    return Err(self.error("unmatched `}`: write `'}'` for a literal brace"));
                }
                '#' if scope.plural => {
                    self.at += 1;
                    push_literal(&mut parts, &mut literal);
                    parts.push(Part::Hash);
                }
                _ => {
                    self.at += next.len_utf8();
                    literal.push(next);
                }
            }
        }

        push_literal(&mut parts, &mut literal);
        Ok(Pattern::new(parts))
    }

    fn quoted(&mut self, literal: &mut String) {
        match self.peek() {
            Some('\'') => {
                self.at += 1;
                literal.push('\'');
            }
            Some('{' | '}' | '#') => {
                while let Some(next) = self.peek() {
                    self.at += next.len_utf8();
                    if next != '\'' {
                        literal.push(next);
                    } else if self.eat('\'') {
                        literal.push('\'');
                    } else {
                        return;
                    }
                }
            }
            _ => literal.push('\''),
        }
    }

    fn placeholder(&mut self, scope: Scope) -> Result<Part, SyntaxError> {
        self.skip_whitespace();
        let name = self.name()?;
        self.skip_whitespace();

        if self.eat('}') {
            return Ok(Part::Arg(name));
        }
        if !self.eat(',') {
            return Err(self.error(format!("expected `,` or `}}` after argument `{name}`")));
        }

        self.skip_whitespace();
        let kind = self.name()?;
        self.skip_whitespace();
        if !self.eat(',') {
            return Err(self.error(format!("expected `,` after `{kind}`")));
        }

        let part = match &*kind {
            "plural" => self.plural(name, PluralKind::Cardinal, scope)?,
            "selectordinal" => self.plural(name, PluralKind::Ordinal, scope)?,
            "select" => self.select(name, scope)?,
            other => {
                return Err(self.error(format!(
                    "unknown placeholder type `{other}`: expected `plural`, `selectordinal` \
                     or `select`"
                )));
            }
        };

        self.skip_whitespace();
        if !self.eat('}') {
            return Err(self.error("expected `}` to close the placeholder"));
        }
        Ok(part)
    }

    fn plural(
        &mut self,
        arg: Box<str>,
        kind: PluralKind,
        scope: Scope,
    ) -> Result<Part, SyntaxError> {
        let mut arms = Vec::new();
        loop {
            self.skip_whitespace();
            if self.peek().is_none_or(|next| next == '}') {
                break;
            }
            let selector = if self.eat('=') {
                let digits = self.name()?;
                let value = digits
                    .parse()
                    .map_err(|_| self.error(format!("`={digits}` is not a whole number")))?;
                Selector::Exact(value)
            } else {
                let keyword = self.name()?;
                let category = PluralCategory::from_name(&keyword).ok_or_else(|| {
                    self.error(format!(
                        "unknown plural category `{keyword}`: expected `zero`, `one`, `two`, \
                         `few`, `many`, `other` or `=<number>`"
                    ))
                })?;
                Selector::Category(category)
            };
            let body = self.arm_body(scope.arm(true))?;
            arms.push(PluralArm { selector, body });
        }

        if !arms
            .iter()
            .any(|arm| arm.selector == Selector::Category(PluralCategory::Other))
        {
            return Err(self.error(format!("plural `{arg}` is missing its `other` arm")));
        }
        Ok(Part::Plural { arg, kind, arms })
    }

    fn select(&mut self, arg: Box<str>, scope: Scope) -> Result<Part, SyntaxError> {
        let mut arms = Vec::new();
        loop {
            self.skip_whitespace();
            if self.peek().is_none_or(|next| next == '}') {
                break;
            }
            let keyword = self.name()?;
            let body = self.arm_body(scope.arm(false))?;
            arms.push(SelectArm { keyword, body });
        }

        if !arms.iter().any(|arm| &*arm.keyword == "other") {
            return Err(self.error(format!("select `{arg}` is missing its `other` arm")));
        }
        Ok(Part::Select { arg, arms })
    }

    fn arm_body(&mut self, scope: Scope) -> Result<Pattern, SyntaxError> {
        self.skip_whitespace();
        if !self.eat('{') {
            return Err(self.error("expected `{` to open an arm"));
        }
        let body = self.pattern(scope)?;
        if !self.eat('}') {
            return Err(self.error("expected `}` to close an arm"));
        }
        Ok(body)
    }

    fn name(&mut self) -> Result<Box<str>, SyntaxError> {
        let end = self
            .rest()
            .find(|c: char| c.is_whitespace() || matches!(c, ',' | '{' | '}' | '#' | '='))
            .unwrap_or(self.rest().len());
        if end == 0 {
            return Err(self.error("expected a name"));
        }
        let name = &self.rest()[..end];
        self.at += end;
        Ok(name.into())
    }
}

fn push_literal(parts: &mut Vec<Part>, literal: &mut String) {
    if !literal.is_empty() {
        parts.push(Part::Literal(std::mem::take(literal).into_boxed_str()));
    }
}
