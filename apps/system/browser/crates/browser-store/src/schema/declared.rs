#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Table,
    Index,
    Trigger,
}

impl Kind {
    pub(super) fn from_type(kind: &str) -> Option<Kind> {
        match kind {
            "table" => Some(Kind::Table),
            "index" => Some(Kind::Index),
            "trigger" => Some(Kind::Trigger),
            _ => None,
        }
    }

    pub(super) fn keyword(self) -> &'static str {
        match self {
            Kind::Table => "TABLE",
            Kind::Index => "INDEX",
            Kind::Trigger => "TRIGGER",
        }
    }
}

#[derive(Debug)]
pub(super) struct Declared {
    pub(super) kind: Kind,
    pub(super) name: &'static str,
    pub(super) sql: &'static str,
    pub(super) depends: Vec<&'static str>,
    pub(super) content: Option<&'static str>,
}

impl Declared {
    pub(super) fn parse(sql: &'static str) -> Declared {
        let words: Vec<&'static str> = sql.split_whitespace().collect();
        let (kind, name) = match words.as_slice() {
            ["CREATE", "TABLE", name, ..] | ["CREATE", "VIRTUAL", "TABLE", name, ..] => {
                (Kind::Table, *name)
            }
            ["CREATE", "INDEX", name, ..] => (Kind::Index, *name),
            ["CREATE", "TRIGGER", name, ..] => (Kind::Trigger, *name),
            _ => unreachable!("schema object {sql:?} is not a table, index or trigger"),
        };
        let content = quoted_after(sql, "content = '");
        let depends = match kind {
            Kind::Table => references(sql).chain(content).collect(),
            Kind::Index | Kind::Trigger => words
                .iter()
                .position(|word| *word == "ON")
                .map(|at| identifier(words[at + 1]))
                .into_iter()
                .collect(),
        };
        Declared {
            kind,
            name,
            sql,
            depends,
            content,
        }
    }

    pub(super) fn matches(&self, stored: &str) -> bool {
        normalized(self.sql) == normalized(stored)
    }
}

fn references(sql: &'static str) -> impl Iterator<Item = &'static str> {
    sql.split("REFERENCES ")
        .skip(1)
        .map(|rest| identifier(rest))
}

fn quoted_after(sql: &'static str, marker: &str) -> Option<&'static str> {
    let rest = &sql[sql.find(marker)? + marker.len()..];
    Some(&rest[..rest.find('\'')?])
}

fn identifier(text: &'static str) -> &'static str {
    let end = text
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(text.len());
    &text[..end]
}

fn normalized(sql: &str) -> String {
    sql.split_whitespace().collect::<Vec<_>>().join(" ")
}
