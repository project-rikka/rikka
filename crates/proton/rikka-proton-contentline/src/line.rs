use std::fmt::{self, Write as _};

use crate::error::{Error, Result};
use crate::folding::FoldingWriter;
use crate::parameter::{Parameter, decode_caret};

const BARE_PARAMETER: &str = "TYPE";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentLine {
    group: Option<String>,
    name: String,
    parameters: Vec<Parameter>,
    value: String,
}

impl ContentLine {
    #[must_use]
    pub fn new(name: impl AsRef<str>, value: impl Into<String>) -> ContentLine {
        ContentLine {
            group: None,
            name: name.as_ref().to_ascii_uppercase(),
            parameters: Vec::new(),
            value: value.into(),
        }
    }

    pub fn parse(line: &str) -> Result<ContentLine> {
        ContentLine::parse_numbered(line, 1)
    }

    pub(crate) fn parse_numbered(line: &str, number: usize) -> Result<ContentLine> {
        let name_end = line
            .find([';', ':'])
            .ok_or(Error::MissingValue { line: number })?;
        let full_name = line[..name_end].trim();
        if full_name.is_empty() {
            return Err(Error::MissingName { line: number });
        }
        let (group, name) = match full_name.split_once('.') {
            Some((group, name)) => (Some(group.to_owned()), name),
            None => (None, full_name),
        };
        if name.is_empty() {
            return Err(Error::MissingName { line: number });
        }
        if let Some(found) = name.chars().find(|character| {
            !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        }) {
            return Err(Error::InvalidName {
                line: number,
                found,
            });
        }

        let mut parameters = Vec::new();
        let mut rest = &line[name_end..];
        while let Some(after) = rest.strip_prefix(';') {
            let (parameter, remaining) = read_parameter(after, number)?;
            if let Some(parameter) = parameter {
                match parameters
                    .iter_mut()
                    .find(|current: &&mut Parameter| current.is(parameter.name()))
                {
                    Some(current) => {
                        for value in parameter.values() {
                            current.push(value.clone());
                        }
                    }
                    None => parameters.push(parameter),
                }
            }
            rest = remaining;
        }

        let value = rest
            .strip_prefix(':')
            .ok_or(Error::MissingValue { line: number })?;

        Ok(ContentLine {
            group,
            name: name.to_ascii_uppercase(),
            parameters,
            value: value.to_owned(),
        })
    }

    #[must_use]
    pub fn with_group(mut self, group: impl Into<String>) -> ContentLine {
        self.group = Some(group.into()).filter(|group: &String| !group.is_empty());
        self
    }

    #[must_use]
    pub fn with_parameter(mut self, parameter: Parameter) -> ContentLine {
        self.set_parameter(parameter);
        self
    }

    #[must_use]
    pub fn group(&self) -> Option<&str> {
        self.group.as_deref()
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn is(&self, name: &str) -> bool {
        self.name.eq_ignore_ascii_case(name)
    }

    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    #[must_use]
    pub fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    #[must_use]
    pub fn parameter(&self, name: &str) -> Option<&Parameter> {
        self.parameters.iter().find(|parameter| parameter.is(name))
    }

    #[must_use]
    pub fn parameter_value(&self, name: &str) -> Option<&str> {
        self.parameter(name).and_then(Parameter::value)
    }

    pub fn set_group(&mut self, group: Option<String>) {
        self.group = group.filter(|group| !group.is_empty());
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
    }

    pub fn set_parameter(&mut self, parameter: Parameter) {
        match self
            .parameters
            .iter_mut()
            .find(|current| current.is(parameter.name()))
        {
            Some(current) => *current = parameter,
            None => self.parameters.push(parameter),
        }
    }

    pub fn remove_parameter(&mut self, name: &str) -> Option<Parameter> {
        let index = self
            .parameters
            .iter()
            .position(|parameter| parameter.is(name))?;
        Some(self.parameters.remove(index))
    }

    #[must_use]
    pub fn unfolded(&self) -> String {
        Unfolded(self).to_string()
    }
}

struct Unfolded<'a>(&'a ContentLine);

impl fmt::Display for Unfolded<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let line = self.0;
        if let Some(group) = &line.group {
            write!(formatter, "{group}.")?;
        }
        formatter.write_str(&line.name)?;
        for parameter in &line.parameters {
            write!(formatter, ";{parameter}")?;
        }
        write!(formatter, ":{}", line.value)
    }
}

impl fmt::Display for ContentLine {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut folded = FoldingWriter::new(formatter);
        write!(folded, "{}", Unfolded(self))?;
        folded.finish()
    }
}

fn read_parameter(text: &str, number: usize) -> Result<(Option<Parameter>, &str)> {
    let name_end = text
        .find(['=', ';', ':'])
        .ok_or(Error::MissingValue { line: number })?;
    let name = text[..name_end].trim();

    if !text[name_end..].starts_with('=') {
        if name.is_empty() {
            return Ok((None, &text[name_end..]));
        }
        return Ok((
            Some(Parameter::new(BARE_PARAMETER, name)),
            &text[name_end..],
        ));
    }
    if name.is_empty() {
        return Err(Error::MissingParameterName { line: number });
    }

    let mut values = Vec::new();
    let mut rest = &text[name_end + 1..];
    loop {
        let (value, remaining) = read_parameter_value(rest, number)?;
        values.push(value);
        match remaining.strip_prefix(',') {
            Some(after) => rest = after,
            None => {
                rest = remaining;
                break;
            }
        }
    }

    Ok((Some(Parameter::with_values(name, values)), rest))
}

fn read_parameter_value(text: &str, number: usize) -> Result<(String, &str)> {
    if let Some(quoted) = text.strip_prefix('"') {
        let end = quoted
            .find('"')
            .ok_or(Error::UnclosedQuote { line: number })?;
        return Ok((decode_caret(&quoted[..end]), &quoted[end + 1..]));
    }
    let end = text.find([',', ';', ':']).unwrap_or(text.len());
    Ok((decode_caret(&text[..end]), &text[end..]))
}
