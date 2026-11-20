#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mention {
    at: usize,
    end: usize,
    query: String,
}

impl Mention {
    pub fn at(&self) -> usize {
        self.at
    }

    pub fn end(&self) -> usize {
        self.end
    }

    pub fn query(&self) -> &str {
        &self.query
    }
}

pub fn mention_at(text: &str, caret: usize) -> Option<Mention> {
    let caret = caret.min(text.len());
    let before = text.get(..caret)?;
    before
        .rmatch_indices('@')
        .filter(|(at, _)| begins(text, *at))
        .map(|(at, _)| token(text, at))
        .find(|typed| caret <= typed.end)
}

pub(crate) fn written_mention(path: &str) -> String {
    match path.chars().all(is_path) {
        true => format!("@{path} "),
        false => format!("@\"{path}\" "),
    }
}

pub(crate) fn opened_mention(path: &str) -> String {
    match path.chars().all(is_path) {
        true => format!("@{path}"),
        false => format!("@\"{path}"),
    }
}

pub fn mentioned(text: &str) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    let mut from = 0;
    while let Some(found) = text[from..].find('@') {
        let at = from + found;
        from = at + 1;
        if !begins(text, at) {
            continue;
        }
        let typed = token(text, at);
        from = typed.end;
        if typed.query.is_empty() || paths.contains(&typed.query) {
            continue;
        }
        paths.push(typed.query);
    }
    paths
}

fn begins(text: &str, at: usize) -> bool {
    at == 0
        || text[..at]
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace)
}

fn token(text: &str, at: usize) -> Mention {
    let after = &text[at + 1..];
    let (query, length) = match after.strip_prefix('"') {
        Some(rest) => {
            let line = rest.split('\n').next().unwrap_or(rest);
            match line.find('"') {
                Some(closing) => (&line[..closing], 1 + closing + 1),
                None => (line, 1 + line.len()),
            }
        }
        None => {
            let length = after.find(|letter| !is_path(letter)).unwrap_or(after.len());
            (&after[..length], length)
        }
    };
    Mention {
        at,
        end: at + 1 + length,
        query: query.to_owned(),
    }
}

fn is_path(letter: char) -> bool {
    letter.is_alphanumeric()
        || matches!(
            letter,
            '_' | '-' | '.' | '/' | '\\' | '(' | ')' | '[' | ']' | '~' | ':'
        )
}
