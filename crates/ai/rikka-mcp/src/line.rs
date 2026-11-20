use std::iter::Peekable;
use std::str::Chars;

pub(crate) fn words(line: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut letters = line.chars().peekable();

    while let Some(letter) = letters.next() {
        match letter {
            '\'' => {
                started = true;
                loop {
                    match letters.next()? {
                        '\'' => break,
                        letter => word.push(letter),
                    }
                }
            }
            '"' => {
                started = true;
                doubly_quoted(&mut letters, &mut word)?;
            }
            '\\' => {
                started = true;
                word.push(letters.next()?);
            }
            letter if letter.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            letter => {
                started = true;
                word.push(letter);
            }
        }
    }

    if started {
        words.push(word);
    }
    Some(words)
}

fn doubly_quoted(letters: &mut Peekable<Chars<'_>>, word: &mut String) -> Option<()> {
    loop {
        match letters.next()? {
            '"' => return Some(()),
            '\\' => match letters.next()? {
                '\n' => {}
                escaped @ ('"' | '\\' | '$' | '`') => word.push(escaped),
                other => {
                    word.push('\\');
                    word.push(other);
                }
            },
            letter => word.push(letter),
        }
    }
}

pub(crate) fn spelled<'a>(words: impl IntoIterator<Item = &'a str>) -> String {
    let mut line = String::new();
    for word in words {
        if !line.is_empty() {
            line.push(' ');
        }
        quote(word, &mut line);
    }
    line
}

fn quote(word: &str, line: &mut String) {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|letter| letter.is_alphanumeric() || "_-./:@%+=,^~".contains(letter));
    if plain {
        line.push_str(word);
        return;
    }

    line.push('\'');
    for letter in word.chars() {
        match letter {
            '\'' => line.push_str("'\\''"),
            letter => line.push(letter),
        }
    }
    line.push('\'');
}
