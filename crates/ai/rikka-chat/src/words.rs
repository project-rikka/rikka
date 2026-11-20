#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Word {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) text: String,
}

pub(crate) fn words(arguments: &str) -> Vec<String> {
    spans(arguments).into_iter().map(|word| word.text).collect()
}

pub(crate) fn spans(arguments: &str) -> Vec<Word> {
    let mut words = Vec::new();
    let mut word: Option<Word> = None;
    let mut quote: Option<char> = None;

    for (at, letter) in arguments.char_indices() {
        let after = at + letter.len_utf8();
        match quote {
            Some(closing) if letter == closing => quote = None,
            Some(_) => {
                if let Some(word) = word.as_mut() {
                    word.text.push(letter);
                }
            }
            None if letter.is_whitespace() => {
                words.extend(word.take());
                continue;
            }
            None if matches!(letter, '"' | '\'') => {
                quote = Some(letter);
                word.get_or_insert_with(|| Word {
                    start: at,
                    end: at,
                    text: String::new(),
                });
            }
            None => word
                .get_or_insert_with(|| Word {
                    start: at,
                    end: at,
                    text: String::new(),
                })
                .text
                .push(letter),
        }
        if let Some(word) = word.as_mut() {
            word.end = after;
        }
    }

    words.extend(word);
    words
}

pub(crate) fn quoted(value: &str) -> String {
    match quoting(value) {
        Some(quote) => format!("{quote}{value}{quote}"),
        None => value.to_owned(),
    }
}

pub(crate) fn opened(value: &str) -> String {
    match quoting(value) {
        Some(quote) => format!("{quote}{value}"),
        None => value.to_owned(),
    }
}

fn quoting(value: &str) -> Option<char> {
    let plain = !value.is_empty()
        && !value
            .chars()
            .any(|letter| letter.is_whitespace() || matches!(letter, '"' | '\''));
    match (plain, value.contains('"')) {
        (true, _) => None,
        (false, true) => Some('\''),
        (false, false) => Some('"'),
    }
}
