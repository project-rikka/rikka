use std::cell::Cell;

const MAX_STEPS: u64 = 1 << 27;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Combinator {
    Descendant,
    Child,
    Next,
    Subsequent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Operator {
    Equals,
    Includes,
    DashMatch,
    Prefix,
    Suffix,
    Substring,
}

#[derive(Clone, Debug, PartialEq)]
enum Condition {
    Id(String),
    Class(String),
    Attribute {
        name: String,
        test: Option<(Operator, String)>,
    },
    Root,
    Only,
    Nth {
        step: i64,
        offset: i64,
        from_end: bool,
    },
    Not(Vec<Compound>),
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Compound {
    tag: Option<String>,
    conditions: Vec<Condition>,
}

#[derive(Clone, Debug)]
pub(crate) struct Selector {
    steps: Vec<(Combinator, Compound)>,
    specificity: u32,
}

pub(crate) trait Scope {
    type Node: Copy;

    fn parent(&self, node: Self::Node) -> Option<Self::Node>;
    fn previous(&self, node: Self::Node) -> Option<Self::Node>;
    fn tag(&self, node: Self::Node) -> &str;
    fn attribute(&self, node: Self::Node, name: &str) -> Option<&str>;
    fn ordinal(&self, node: Self::Node) -> (usize, usize);
}

#[derive(Debug, Default)]
pub(crate) struct Work {
    steps: Cell<u64>,
}

impl Work {
    pub(crate) fn exhausted(&self) -> bool {
        self.steps.get() > MAX_STEPS
    }

    fn step(&self) -> bool {
        let steps = self.steps.get() + 1;
        self.steps.set(steps);
        steps <= MAX_STEPS
    }
}

impl Selector {
    pub(crate) fn parse(text: &str) -> Option<Selector> {
        let mut stream = Tokens::new(text.trim());
        let mut steps = Vec::new();
        let mut combinator = Combinator::Descendant;
        loop {
            steps.push((combinator, stream.compound()?));
            let spaced = stream.skip_spaces();
            combinator = match stream.peek() {
                None => break,
                Some('>') => Combinator::Child,
                Some('+') => Combinator::Next,
                Some('~') => Combinator::Subsequent,
                Some(_) if spaced => {
                    combinator = Combinator::Descendant;
                    continue;
                }
                Some(_) => return None,
            };
            stream.advance(1);
            stream.skip_spaces();
        }
        let specificity = steps
            .iter()
            .map(|(_, compound)| compound.specificity())
            .sum();
        Some(Selector { steps, specificity })
    }

    pub(crate) fn specificity(&self) -> u32 {
        self.specificity
    }

    pub(crate) fn matches<S: Scope>(&self, scope: &S, node: S::Node, work: &Work) -> bool {
        matches_from(&self.steps, scope, node, work)
    }
}

fn matches_from<S: Scope>(
    steps: &[(Combinator, Compound)],
    scope: &S,
    node: S::Node,
    work: &Work,
) -> bool {
    let Some(((combinator, compound), earlier)) = steps.split_last() else {
        return true;
    };
    if !compound.fits(scope, node, work) {
        return false;
    }
    if earlier.is_empty() {
        return true;
    }
    let mut candidate = match combinator {
        Combinator::Descendant | Combinator::Child => scope.parent(node),
        Combinator::Next | Combinator::Subsequent => scope.previous(node),
    };
    while let Some(next) = candidate {
        if !work.step() {
            return false;
        }
        if matches_from(earlier, scope, next, work) {
            return true;
        }
        candidate = match combinator {
            Combinator::Descendant => scope.parent(next),
            Combinator::Subsequent => scope.previous(next),
            Combinator::Child | Combinator::Next => None,
        };
    }
    false
}

impl Compound {
    fn specificity(&self) -> u32 {
        let own: u32 = self
            .conditions
            .iter()
            .map(|condition| match condition {
                Condition::Id(_) => 10_000,
                Condition::Not(list) => list.iter().map(Compound::specificity).max().unwrap_or(0),
                _ => 100,
            })
            .sum();
        own + u32::from(self.tag.is_some())
    }

    fn fits<S: Scope>(&self, scope: &S, node: S::Node, work: &Work) -> bool {
        if !work.step() {
            return false;
        }
        if self
            .tag
            .as_deref()
            .is_some_and(|tag| tag != scope.tag(node))
        {
            return false;
        }
        self.conditions
            .iter()
            .all(|condition| condition.holds(scope, node, work))
    }
}

impl Condition {
    fn holds<S: Scope>(&self, scope: &S, node: S::Node, work: &Work) -> bool {
        match self {
            Condition::Id(id) => scope.attribute(node, "id") == Some(id.as_str()),
            Condition::Class(class) => scope
                .attribute(node, "class")
                .is_some_and(|classes| classes.split_ascii_whitespace().any(|own| own == class)),
            Condition::Attribute { name, test } => {
                let Some(value) = scope.attribute(node, name) else {
                    return false;
                };
                let Some((operator, expected)) = test else {
                    return true;
                };
                let expected = expected.as_str();
                match operator {
                    Operator::Equals => value == expected,
                    Operator::Includes => {
                        value.split_ascii_whitespace().any(|word| word == expected)
                    }
                    Operator::DashMatch => {
                        value == expected
                            || value
                                .strip_prefix(expected)
                                .is_some_and(|rest| rest.starts_with('-'))
                    }
                    Operator::Prefix => !expected.is_empty() && value.starts_with(expected),
                    Operator::Suffix => !expected.is_empty() && value.ends_with(expected),
                    Operator::Substring => !expected.is_empty() && value.contains(expected),
                }
            }
            Condition::Root => scope.parent(node).is_none(),
            Condition::Only => scope.ordinal(node).1 == 1,
            Condition::Nth {
                step,
                offset,
                from_end,
            } => {
                let (ordinal, count) = scope.ordinal(node);
                let position = match from_end {
                    true => (count - ordinal + 1) as i64,
                    false => ordinal as i64,
                };
                match *step {
                    0 => position == *offset,
                    step => {
                        let distance = position - offset;
                        distance % step == 0 && distance / step >= 0
                    }
                }
            }
            Condition::Not(list) => !list.iter().any(|compound| compound.fits(scope, node, work)),
        }
    }
}

struct Tokens<'a> {
    text: &'a str,
    at: usize,
}

impl<'a> Tokens<'a> {
    fn new(text: &'a str) -> Tokens<'a> {
        Tokens { text, at: 0 }
    }

    fn rest(&self) -> &'a str {
        &self.text[self.at..]
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn advance(&mut self, bytes: usize) {
        self.at += bytes;
    }

    fn skip_spaces(&mut self) -> bool {
        let rest = self.rest();
        let trimmed = rest.trim_start();
        self.at += rest.len() - trimmed.len();
        rest.len() != trimmed.len()
    }

    fn identifier(&mut self) -> Option<&'a str> {
        let rest = self.rest();
        let length = rest
            .find(|character: char| {
                !(character.is_alphanumeric()
                    || matches!(character, '-' | '_' | '\\')
                    || !character.is_ascii())
            })
            .unwrap_or(rest.len());
        self.at += length;
        (length > 0).then(|| &rest[..length])
    }

    fn compound(&mut self) -> Option<Compound> {
        let mut compound = Compound::default();
        let mut universal = false;
        match self.peek()? {
            '*' => {
                self.advance(1);
                universal = true;
            }
            character if character.is_alphabetic() || character == '_' => {
                compound.tag = Some(self.identifier()?.to_owned());
            }
            _ => {}
        }
        while let Some(character) = self.peek() {
            let condition = match character {
                '#' => {
                    self.advance(1);
                    Condition::Id(self.identifier()?.to_owned())
                }
                '.' => {
                    self.advance(1);
                    Condition::Class(self.identifier()?.to_owned())
                }
                '[' => {
                    self.advance(1);
                    self.attribute()?
                }
                ':' => {
                    self.advance(1);
                    self.pseudo_class()?
                }
                _ => break,
            };
            compound.conditions.push(condition);
        }
        let named = universal || compound.tag.is_some() || !compound.conditions.is_empty();
        named.then_some(compound)
    }

    fn attribute(&mut self) -> Option<Condition> {
        self.skip_spaces();
        let name = self.identifier()?.to_owned();
        self.skip_spaces();
        if self.peek() == Some(']') {
            self.advance(1);
            return Some(Condition::Attribute { name, test: None });
        }
        let operators = [
            ("=", Operator::Equals),
            ("~=", Operator::Includes),
            ("|=", Operator::DashMatch),
            ("^=", Operator::Prefix),
            ("$=", Operator::Suffix),
            ("*=", Operator::Substring),
        ];
        let (symbol, operator) = operators
            .into_iter()
            .find(|(symbol, _)| self.rest().starts_with(symbol))?;
        self.advance(symbol.len());
        self.skip_spaces();
        let value = match self.peek()? {
            quote @ ('"' | '\'') => {
                let body = &self.rest()[1..];
                let end = body.find(quote)?;
                self.advance(end + 2);
                body[..end].to_owned()
            }
            _ => self.identifier()?.to_owned(),
        };
        self.skip_spaces();
        if self.peek() != Some(']') {
            return None;
        }
        self.advance(1);
        Some(Condition::Attribute {
            name,
            test: Some((operator, value)),
        })
    }

    fn pseudo_class(&mut self) -> Option<Condition> {
        let name = self.identifier()?.to_ascii_lowercase();
        let first = Condition::Nth {
            step: 0,
            offset: 1,
            from_end: false,
        };
        let last = Condition::Nth {
            step: 0,
            offset: 1,
            from_end: true,
        };
        match name.as_str() {
            "root" => Some(Condition::Root),
            "first-child" => Some(first),
            "last-child" => Some(last),
            "only-child" => Some(Condition::Only),
            "nth-child" | "nth-last-child" => {
                let argument = self.argument()?;
                let (step, offset) = nth(argument)?;
                Some(Condition::Nth {
                    step,
                    offset,
                    from_end: name == "nth-last-child",
                })
            }
            "not" => {
                let argument = self.argument()?;
                let list = argument
                    .split(',')
                    .map(|part| {
                        let mut inner = Tokens::new(part.trim());
                        let compound = inner.compound()?;
                        inner.rest().is_empty().then_some(compound)
                    })
                    .collect::<Option<Vec<Compound>>>()?;
                Some(Condition::Not(list))
            }
            _ => None,
        }
    }

    fn argument(&mut self) -> Option<&'a str> {
        let rest = self.rest().strip_prefix('(')?;
        let mut depth = 0usize;
        for (at, character) in rest.char_indices() {
            match character {
                '(' => depth += 1,
                ')' if depth == 0 => {
                    self.advance(at + 2);
                    return Some(&rest[..at]);
                }
                ')' => depth -= 1,
                _ => {}
            }
        }
        None
    }
}

fn nth(argument: &str) -> Option<(i64, i64)> {
    let text: String = argument
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    match text.as_str() {
        "odd" => return Some((2, 1)),
        "even" => return Some((2, 0)),
        _ => {}
    }
    let Some((step, offset)) = text.split_once('n') else {
        return Some((0, text.parse().ok()?));
    };
    let step = match step {
        "" | "+" => 1,
        "-" => -1,
        step => step.parse().ok()?,
    };
    let offset = match offset {
        "" => 0,
        offset => offset.parse().ok()?,
    };
    Some((step, offset))
}
