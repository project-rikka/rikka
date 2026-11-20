#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    oid: String,
    short: String,
    author: String,
    at: u64,
    summary: String,
    body: String,
}

impl Commit {
    #[must_use]
    pub fn oid(&self) -> &str {
        &self.oid
    }

    #[must_use]
    pub fn short(&self) -> &str {
        &self.short
    }

    #[must_use]
    pub fn author(&self) -> &str {
        &self.author
    }

    #[must_use]
    pub fn at(&self) -> u64 {
        self.at
    }

    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    #[must_use]
    pub fn body(&self) -> &str {
        &self.body
    }
}

const FIELD: char = '\u{1f}';
const RECORD: char = '\u{1e}';

pub(crate) const FORMAT: &str =
    "--format=format:%H\u{1f}%h\u{1f}%an\u{1f}%at\u{1f}%s\u{1f}%b\u{1e}";

pub(crate) fn read(output: &str) -> Vec<Commit> {
    output.split(RECORD).filter_map(one).collect()
}

fn one(record: &str) -> Option<Commit> {
    let mut fields = record.trim_start_matches('\n').split(FIELD);
    let oid = fields.next().filter(|oid| !oid.is_empty())?;
    Some(Commit {
        oid: oid.to_owned(),
        short: fields.next()?.to_owned(),
        author: fields.next()?.to_owned(),
        at: fields.next()?.parse().ok()?,
        summary: fields.next()?.to_owned(),
        body: fields.next().unwrap_or_default().trim().to_owned(),
    })
}
