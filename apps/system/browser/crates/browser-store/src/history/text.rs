use rusqlite::types::Value;

use crate::records::{like_pattern, push_like};

const TRIGRAM: usize = 3;

#[derive(Debug, Default)]
pub(crate) struct TextFilter {
    phrases: Vec<String>,
    fragments: Vec<String>,
}

impl TextFilter {
    pub(crate) fn parse(text: &str) -> Self {
        let mut filter = Self::default();
        for term in text.split_whitespace() {
            if term.chars().count() >= TRIGRAM {
                filter
                    .phrases
                    .push(format!("\"{}\"", term.replace('"', "\"\"")));
            } else {
                filter.fragments.push(like_pattern(term));
            }
        }
        filter
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.phrases.is_empty() && self.fragments.is_empty()
    }

    pub(crate) fn push_sql(&self, sql: &mut String, values: &mut Vec<Value>) {
        if !self.phrases.is_empty() {
            sql.push_str(" AND p.id IN (SELECT rowid FROM places_text WHERE places_text MATCH ?)");
            values.push(Value::Text(self.phrases.join(" AND ")));
        }
        for fragment in &self.fragments {
            push_like(sql, values, "p.", fragment);
        }
    }
}
