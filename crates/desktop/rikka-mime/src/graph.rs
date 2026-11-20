use std::collections::HashMap;

use crate::mime_type::MimeType;

const UNSTREAMABLE: [&str; 2] = ["inode", "x-scheme-handler"];

#[derive(Debug, Default)]
pub(crate) struct Graph {
    aliases: HashMap<MimeType, MimeType>,
    parents: HashMap<MimeType, Vec<MimeType>>,
}

impl Graph {
    pub(crate) fn extend_aliases(&mut self, text: &str) {
        for (alias, canonical) in text.lines().filter_map(pair) {
            self.aliases.entry(alias).or_insert(canonical);
        }
    }

    pub(crate) fn extend_subclasses(&mut self, text: &str) {
        for (child, parent) in text.lines().filter_map(pair) {
            self.parents.entry(child).or_default().push(parent);
        }
    }

    pub(crate) fn resolve(&self, mime: &MimeType) -> MimeType {
        self.aliases
            .get(mime)
            .cloned()
            .unwrap_or_else(|| mime.clone())
    }

    pub(crate) fn ancestry(&self, mime: &MimeType) -> Vec<MimeType> {
        let mut order = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::from([self.resolve(mime)]);
        while let Some(current) = queue.pop_front() {
            if !seen.insert(current.clone()) {
                continue;
            }
            if let Some(parents) = self.parents.get(&current) {
                queue.extend(parents.iter().map(|parent| self.resolve(parent)));
            }
            order.push(current);
        }
        let streamable = order
            .first()
            .is_some_and(|first| !UNSTREAMABLE.contains(&first.kind()));
        let text = order.first().is_some_and(|first| first.kind() == "text");
        let implied = [(MimeType::TEXT, text), (MimeType::UNKNOWN, streamable)];
        for (implied, applies) in implied {
            let implied = MimeType::well_known(implied);
            if applies && !order.contains(&implied) {
                order.push(implied);
            }
        }
        order
    }
}

fn pair(line: &str) -> Option<(MimeType, MimeType)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (left, right) = line.split_once(char::is_whitespace)?;
    Some((MimeType::parse(left)?, MimeType::parse(right.trim())?))
}
