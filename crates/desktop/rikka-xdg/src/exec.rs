#[derive(Clone, Debug)]
pub struct Target {
    pub path: String,
    pub uri: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expansion {
    Word(String),
    Args(Vec<String>),
    Nothing,
    Unknown,
}

pub trait Codes {
    fn expand(&self, code: char) -> Expansion;
}

impl<F> Codes for F
where
    F: Fn(char) -> Expansion,
{
    fn expand(&self, code: char) -> Expansion {
        self(code)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FieldCodes<'a> {
    pub paths: &'a [&'a str],
    pub uris: &'a [&'a str],
    pub name: &'a str,
    pub icon: Option<&'a str>,
    pub location: Option<&'a str>,
}

impl Codes for FieldCodes<'_> {
    fn expand(&self, code: char) -> Expansion {
        let listed = |all: &[&str]| match all {
            [] => Expansion::Nothing,
            [first, ..] => Expansion::Word((*first).to_owned()),
        };
        match code {
            'f' => listed(self.paths),
            'u' => listed(self.uris),
            'F' => Expansion::Args(self.paths.iter().map(|one| (*one).to_owned()).collect()),
            'U' => Expansion::Args(self.uris.iter().map(|one| (*one).to_owned()).collect()),
            'c' => Expansion::Word(self.name.to_owned()),
            'k' => match self.location {
                Some(location) => Expansion::Word(location.to_owned()),
                None => Expansion::Nothing,
            },
            'i' => match self.icon {
                Some(icon) => Expansion::Args(vec!["--icon".to_owned(), icon.to_owned()]),
                None => Expansion::Nothing,
            },
            'd' | 'D' | 'n' | 'N' | 'v' | 'm' => Expansion::Nothing,
            _ => Expansion::Unknown,
        }
    }
}

#[derive(Debug)]
pub struct Exec;

impl Exec {
    #[must_use]
    pub fn expand(exec: &str, codes: &impl Codes) -> Vec<String> {
        let mut parts: Vec<String> = Vec::new();
        let mut word = String::new();
        let mut started = false;
        let mut quoted = false;
        let mut chars = exec.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                '"' => {
                    quoted = !quoted;
                    started = true;
                }
                '\\' if quoted => {
                    match chars.peek() {
                        Some(&escaped @ ('"' | '`' | '$' | '\\')) => {
                            word.push(escaped);
                            chars.next();
                        }
                        _ => word.push('\\'),
                    }
                    started = true;
                }
                ch if ch.is_whitespace() && !quoted => {
                    if started || !word.is_empty() {
                        parts.push(std::mem::take(&mut word));
                        started = false;
                    }
                }
                '%' => match chars.next() {
                    Some('%') => {
                        word.push('%');
                        started = true;
                    }
                    Some(code) => match codes.expand(code) {
                        Expansion::Word(text) => {
                            word.push_str(&text);
                            started = true;
                        }
                        Expansion::Args(args) => {
                            if started || !word.is_empty() {
                                parts.push(std::mem::take(&mut word));
                                started = false;
                            }
                            parts.extend(args);
                        }
                        Expansion::Nothing => {}
                        Expansion::Unknown => {
                            word.push('%');
                            word.push(code);
                            started = true;
                        }
                    },
                    None => {
                        word.push('%');
                        started = true;
                    }
                },
                ch => {
                    word.push(ch);
                    started = true;
                }
            }
        }
        if started || !word.is_empty() {
            parts.push(word);
        }
        parts
    }

    #[must_use]
    pub fn words(exec: &str) -> Vec<String> {
        Self::expand(
            exec,
            &FieldCodes {
                paths: &[],
                uris: &[],
                name: "",
                icon: None,
                location: None,
            },
        )
    }

    pub(crate) fn takes_many(exec: &str) -> bool {
        let mut chars = exec.chars();
        while let Some(ch) = chars.next() {
            if ch != '%' {
                continue;
            }
            match chars.next() {
                Some('F' | 'U') => return true,
                Some('%') | Some(_) => {}
                None => return false,
            }
        }
        false
    }

    pub(crate) fn program(exec: &str) -> Option<String> {
        let words = Self::words(exec);
        let mut words = words.iter().peekable();
        while let Some(word) = words.next() {
            if word == "env" || word.ends_with("/env") {
                while words.peek().is_some_and(|next| next.contains('=')) {
                    words.next();
                }
                continue;
            }
            if word.is_empty() || word.starts_with('%') || word.contains('=') {
                continue;
            }
            return Some(word.rsplit('/').next()?.to_owned());
        }
        None
    }
}
