use crate::error::{Error, Result};

pub(crate) const CODEPOINT_COUNT: usize = 0x11_0000;

const DEFAULT_PREFIX: &str = "# @missing:";

#[derive(Debug)]
pub(crate) struct Record<'a> {
    pub(crate) first: u32,
    pub(crate) last: u32,
    pub(crate) fields: Vec<&'a str>,
    pub(crate) line: usize,
}

impl Record<'_> {
    pub(crate) fn field(&self, index: usize) -> &str {
        self.fields.get(index).copied().unwrap_or("")
    }
}

#[derive(Debug)]
pub(crate) struct File<'a> {
    name: &'a str,
    text: &'a str,
}

impl<'a> File<'a> {
    pub(crate) fn new(name: &'a str, text: &'a str) -> Self {
        Self { name, text }
    }

    pub(crate) fn error(&self, line: usize, message: impl Into<String>) -> Error {
        Error::Parse {
            file: self.name.to_owned(),
            line,
            message: message.into(),
        }
    }

    pub(crate) fn records(&self) -> Result<Vec<Record<'a>>> {
        let mut records = Vec::new();
        for (index, line) in self.text.lines().enumerate() {
            let data = line.split('#').next().unwrap_or("").trim();
            if !data.is_empty() {
                records.push(self.record(index + 1, data)?);
            }
        }
        Ok(records)
    }

    pub(crate) fn defaults(&self) -> Result<Vec<Record<'a>>> {
        let mut records = Vec::new();
        for (index, line) in self.text.lines().enumerate() {
            if let Some(data) = line.strip_prefix(DEFAULT_PREFIX) {
                records.push(self.record(index + 1, data.trim())?);
            }
        }
        Ok(records)
    }

    pub(crate) fn fill<T: Copy>(
        &self,
        column: &mut [T],
        mut value: impl FnMut(&Record<'a>) -> Result<Option<T>>,
    ) -> Result<()> {
        for record in self.defaults()?.iter().chain(self.records()?.iter()) {
            if let Some(value) = value(record)? {
                column[record.first as usize..=record.last as usize].fill(value);
            }
        }
        Ok(())
    }

    fn record(&self, line: usize, data: &'a str) -> Result<Record<'a>> {
        let mut fields: Vec<&str> = data.split(';').map(str::trim).collect();
        let range = fields.remove(0);
        let (first, last) = parse_range(range)
            .ok_or_else(|| self.error(line, format!("invalid code point range `{range}`")))?;
        Ok(Record {
            first,
            last,
            fields,
            line,
        })
    }
}

pub(crate) fn parse_codepoint(text: &str) -> Option<u32> {
    u32::from_str_radix(text.trim(), 16)
        .ok()
        .filter(|&codepoint| (codepoint as usize) < CODEPOINT_COUNT)
}

fn parse_range(text: &str) -> Option<(u32, u32)> {
    match text.split_once("..") {
        Some((first, last)) => Some((parse_codepoint(first)?, parse_codepoint(last)?)),
        None => parse_codepoint(text).map(|codepoint| (codepoint, codepoint)),
    }
}
