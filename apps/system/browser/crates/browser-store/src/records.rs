use std::sync::Arc;

use rusqlite::Row;
use rusqlite::types::Value;

use crate::ids::SpaceId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabRecord {
    pub url: String,
    pub title: String,
    pub state: Option<Arc<[u8]>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restored {
    pub space: SpaceId,
    pub tab: TabRecord,
}

impl Restored {
    pub(crate) fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let state: Option<Vec<u8>> = row.get(3)?;
        Ok(Self {
            space: row.get(0)?,
            tab: TabRecord {
                url: row.get(1)?,
                title: row.get(2)?,
                state: state.map(Arc::from),
            },
        })
    }
}

pub(crate) fn like_pattern(text: &str) -> String {
    let mut pattern = String::with_capacity(text.len() + 2);
    pattern.push('%');
    for c in text.chars() {
        if matches!(c, '%' | '_' | '\\') {
            pattern.push('\\');
        }
        pattern.push(c);
    }
    pattern.push('%');
    pattern
}

pub(crate) fn push_like(sql: &mut String, values: &mut Vec<Value>, table: &str, pattern: &str) {
    sql.push_str(&format!(
        " AND ({table}title LIKE ? ESCAPE '\\' OR {table}url LIKE ? ESCAPE '\\')"
    ));
    values.push(Value::Text(pattern.to_owned()));
    values.push(Value::Text(pattern.to_owned()));
}

pub(crate) fn count(limit: usize) -> i64 {
    i64::try_from(limit).unwrap_or(i64::MAX)
}
