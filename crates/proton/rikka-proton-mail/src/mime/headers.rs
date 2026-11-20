use crate::address::MailAddress;
use crate::mime::{address_list, decode_charset, encoded_words};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderField {
    name: String,
    raw: Vec<u8>,
}

impl HeaderField {
    pub(crate) fn new(name: impl Into<String>, value: &str) -> HeaderField {
        HeaderField {
            name: name.into(),
            raw: format!(" {value}").into_bytes(),
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn unfolded(&self) -> String {
        let text = decode_charset(&self.raw, None);
        let mut unfolded = String::with_capacity(text.len());
        for line in text.split('\n') {
            unfolded.push_str(line.strip_suffix('\r').unwrap_or(line));
        }
        unfolded.trim().to_owned()
    }

    #[must_use]
    pub fn value(&self) -> String {
        encoded_words::decode(&self.unfolded())
    }

    pub(crate) fn write_to(&self, output: &mut Vec<u8>) {
        output.extend_from_slice(self.name.as_bytes());
        output.push(b':');
        output.extend_from_slice(&self.raw);
        output.extend_from_slice(b"\r\n");
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Headers {
    fields: Vec<HeaderField>,
}

impl Headers {
    pub(crate) fn parse(block: &[u8]) -> Headers {
        let mut fields: Vec<HeaderField> = Vec::new();

        for line in block.split(|byte| *byte == b'\n') {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if line.is_empty() {
                continue;
            }

            if line[0] == b' ' || line[0] == b'\t' {
                if let Some(last) = fields.last_mut() {
                    last.raw.extend_from_slice(b"\r\n");
                    last.raw.extend_from_slice(line);
                }
                continue;
            }

            if let Some(colon) = field_name_end(line) {
                fields.push(HeaderField {
                    name: String::from_utf8_lossy(&line[..colon])
                        .trim_end()
                        .to_owned(),
                    raw: line[colon + 1..].to_vec(),
                });
            }
        }

        Headers { fields }
    }

    pub fn iter(&self) -> std::slice::Iter<'_, HeaderField> {
        self.fields.iter()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    #[must_use]
    pub fn field(&self, name: &str) -> Option<&HeaderField> {
        self.fields
            .iter()
            .find(|field| field.name.eq_ignore_ascii_case(name))
    }

    pub fn fields<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a HeaderField> + 'a {
        self.fields
            .iter()
            .filter(move |field| field.name.eq_ignore_ascii_case(name))
    }

    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.field(name).is_some()
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<String> {
        self.field(name).map(HeaderField::value)
    }

    #[must_use]
    pub fn unfolded(&self, name: &str) -> Option<String> {
        self.field(name).map(HeaderField::unfolded)
    }

    #[must_use]
    pub fn addresses(&self, name: &str) -> Vec<MailAddress> {
        self.fields(name)
            .flat_map(|field| address_list::parse(&field.unfolded()))
            .collect()
    }

    #[must_use]
    pub fn message_id(&self, name: &str) -> Option<String> {
        self.unfolded(name).map(|value| {
            value
                .trim()
                .trim_start_matches('<')
                .trim_end_matches('>')
                .to_owned()
        })
    }

    pub(crate) fn remove(&mut self, name: &str) {
        self.fields
            .retain(|field| !field.name.eq_ignore_ascii_case(name));
    }

    pub(crate) fn set(&mut self, name: &str, value: &str) {
        let replacement = HeaderField::new(name, value);
        match self
            .fields
            .iter()
            .position(|field| field.name.eq_ignore_ascii_case(name))
        {
            Some(first) => {
                self.fields[first] = replacement;
                let mut count = 0usize;
                self.fields.retain(|field| {
                    if !field.name.eq_ignore_ascii_case(name) {
                        return true;
                    }
                    count += 1;
                    count == 1
                });
            }
            None => self.fields.push(replacement),
        }
    }

    pub(crate) fn push(&mut self, field: HeaderField) {
        self.fields.push(field);
    }

    pub(crate) fn write_to(&self, output: &mut Vec<u8>) {
        for field in &self.fields {
            field.write_to(output);
        }
        output.extend_from_slice(b"\r\n");
    }
}

impl<'a> IntoIterator for &'a Headers {
    type Item = &'a HeaderField;
    type IntoIter = std::slice::Iter<'a, HeaderField>;

    fn into_iter(self) -> Self::IntoIter {
        self.fields.iter()
    }
}

pub(crate) fn field_name_end(line: &[u8]) -> Option<usize> {
    let colon = line.iter().position(|byte| *byte == b':')?;
    let name = &line[..colon];
    let trimmed = name
        .iter()
        .rposition(|byte| *byte != b' ' && *byte != b'\t')
        .map_or(0, |last| last + 1);

    let valid = trimmed > 0
        && name[..trimmed]
            .iter()
            .all(|byte| (33..=126).contains(byte) && *byte != b':');

    valid.then_some(colon)
}
