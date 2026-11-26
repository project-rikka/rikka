use crate::error::{Error, Result};

pub fn split_arguments(text: &str) -> Result<Vec<String>> {
    let mut arguments = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut quote: Option<char> = None;
    let mut characters = text.chars();
    while let Some(character) = characters.next() {
        match (quote, character) {
            (Some(open), close) if open == close => quote = None,
            (Some('"'), '\\') => match characters.next() {
                Some(escaped) => current.push(escaped),
                None => return Err(Error::Quote),
            },
            (Some(_), other) => current.push(other),
            (None, '"' | '\'') => {
                quote = Some(character);
                started = true;
            }
            (None, whitespace) if whitespace.is_whitespace() => {
                if started {
                    arguments.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            (None, other) => {
                current.push(other);
                started = true;
            }
        }
    }
    if quote.is_some() {
        return Err(Error::Quote);
    }
    if started {
        arguments.push(current);
    }
    Ok(arguments)
}
