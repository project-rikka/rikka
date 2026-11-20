use std::fmt::{self, Write as _};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    name: String,
    values: Vec<String>,
}

impl Parameter {
    #[must_use]
    pub fn new(name: impl AsRef<str>, value: impl Into<String>) -> Parameter {
        Parameter::with_values(name, [value])
    }

    #[must_use]
    pub fn with_values<V: Into<String>>(
        name: impl AsRef<str>,
        values: impl IntoIterator<Item = V>,
    ) -> Parameter {
        Parameter {
            name: name.as_ref().to_ascii_uppercase(),
            values: values.into_iter().map(Into::into).collect(),
        }
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
    pub fn values(&self) -> &[String] {
        &self.values
    }

    #[must_use]
    pub fn value(&self) -> Option<&str> {
        self.values.first().map(String::as_str)
    }

    #[must_use]
    pub fn has_value(&self, value: &str) -> bool {
        self.values
            .iter()
            .any(|member| member.eq_ignore_ascii_case(value))
    }

    pub fn push(&mut self, value: impl Into<String>) {
        self.values.push(value.into());
    }
}

impl fmt::Display for Parameter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}=", self.name)?;
        for (index, value) in self.values.iter().enumerate() {
            if index > 0 {
                formatter.write_char(',')?;
            }
            write_value(value, formatter)?;
        }
        Ok(())
    }
}

fn write_value(value: &str, out: &mut impl fmt::Write) -> fmt::Result {
    let quoted = value
        .chars()
        .any(|character| matches!(character, ':' | ';' | ',') || character.is_control());
    if quoted {
        out.write_char('"')?;
    }
    for character in value.chars() {
        match character {
            '^' => out.write_str("^^")?,
            '"' => out.write_str("^'")?,
            '\n' => out.write_str("^n")?,
            '\r' => {}
            other => out.write_char(other)?,
        }
    }
    if quoted {
        out.write_char('"')?;
    }
    Ok(())
}

pub(crate) fn decode_caret(raw: &str) -> String {
    let mut decoded = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();

    while let Some(character) = chars.next() {
        if character != '^' {
            decoded.push(character);
            continue;
        }
        match chars.peek() {
            Some('n') => {
                chars.next();
                decoded.push('\n');
            }
            Some('^') => {
                chars.next();
                decoded.push('^');
            }
            Some('\'') => {
                chars.next();
                decoded.push('"');
            }
            _ => decoded.push('^'),
        }
    }

    decoded
}
