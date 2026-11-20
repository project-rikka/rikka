use std::collections::BTreeMap;

use crate::error::{Error, Result};
use crate::punycode;

const BEGIN_ICANN: &str = "// ===BEGIN ICANN DOMAINS===";
const END_ICANN: &str = "// ===END ICANN DOMAINS===";
const BEGIN_PRIVATE: &str = "// ===BEGIN PRIVATE DOMAINS===";
const END_PRIVATE: &str = "// ===END PRIVATE DOMAINS===";
const ACE_PREFIX: &str = "xn--";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Section {
    Icann,
    Private,
}

impl Section {
    pub(crate) fn shift(self) -> u8 {
        match self {
            Section::Icann => 0,
            Section::Private => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Normal,
    Wildcard,
    Exception,
}

impl Kind {
    pub(crate) fn bit(self) -> u8 {
        match self {
            Kind::Normal => 1,
            Kind::Wildcard => 2,
            Kind::Exception => 4,
        }
    }
}

pub(crate) type Rules = BTreeMap<String, u8>;

pub(crate) fn parse(text: &str) -> Result<Rules> {
    let mut rules = Rules::new();
    let mut section = None;
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        match line {
            BEGIN_ICANN => section = Some(Section::Icann),
            BEGIN_PRIVATE => section = Some(Section::Private),
            END_ICANN | END_PRIVATE => section = None,
            _ => {}
        }
        let Some(rule) = line.split_whitespace().next() else {
            continue;
        };
        if rule.starts_with("//") {
            continue;
        }
        let number = index + 1;
        let Some(section) = section else {
            return Err(Error::Unsectioned {
                line: number,
                rule: rule.to_owned(),
            });
        };
        let (kind, name) = classify(rule).ok_or_else(|| Error::MalformedRule {
            line: number,
            rule: rule.to_owned(),
        })?;
        let flag = kind.bit() << section.shift();
        for form in forms(name) {
            *rules.entry(form).or_default() |= flag;
        }
    }
    if rules.is_empty() {
        return Err(Error::Empty);
    }
    Ok(rules)
}

fn classify(rule: &str) -> Option<(Kind, &str)> {
    let (kind, name) = if let Some(name) = rule.strip_prefix('!') {
        (Kind::Exception, name)
    } else if let Some(name) = rule.strip_prefix("*.") {
        (Kind::Wildcard, name)
    } else {
        (Kind::Normal, rule)
    };
    let valid = name.split('.').all(|label| {
        !label.is_empty()
            && label != "*"
            && !label.contains(['!', '/'])
            && !label.bytes().any(|byte| byte.is_ascii_uppercase())
    });
    valid.then_some((kind, name))
}

fn forms(name: &str) -> Vec<String> {
    let mut forms = vec![name.to_owned()];
    if !name.is_ascii() {
        let ascii: Vec<String> = name
            .split('.')
            .map(|label| match label.is_ascii() {
                true => label.to_owned(),
                false => format!("{ACE_PREFIX}{}", punycode::encode(label)),
            })
            .collect();
        forms.push(ascii.join("."));
    }
    forms
}
