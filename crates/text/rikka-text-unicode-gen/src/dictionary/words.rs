use crate::error::{Error, Result};

pub(crate) const WINDOW: usize = 256;

const JOINERS: [char; 2] = ['\u{200C}', '\u{200D}'];

#[derive(Debug)]
pub(crate) struct WordList {
    pub(crate) symbols: [u8; WINDOW],
    pub(crate) words: Vec<Vec<u8>>,
}

fn slot(character: char, window: u32) -> Option<usize> {
    if let Some(joiner) = JOINERS.iter().position(|&joiner| joiner == character) {
        return Some(WINDOW + joiner);
    }
    (character as u32)
        .checked_sub(window)
        .map(|offset| offset as usize)
        .filter(|&offset| offset < WINDOW)
}

pub(crate) fn parse(file: &str, contents: &str, window: u32) -> Result<WordList> {
    let mut entries: Vec<Vec<usize>> = Vec::new();
    for (index, line) in contents.lines().enumerate() {
        let line = line.trim_start_matches('\u{FEFF}');
        let line = line.split('#').next().unwrap_or_default().trim_end();
        if line.is_empty() {
            continue;
        }
        let word = line.split(char::is_whitespace).next().unwrap_or_default();
        let invalid = |message: String| Error::Parse {
            file: file.to_owned(),
            line: index + 1,
            message,
        };
        if word.is_empty() {
            return Err(invalid("the line has no word".into()));
        }
        let slots = word
            .chars()
            .map(|character| {
                slot(character, window).ok_or_else(|| {
                    invalid(format!(
                        "U+{:04X} is outside the dictionary window",
                        character as u32
                    ))
                })
            })
            .collect::<Result<Vec<usize>>>()?;
        entries.push(slots);
    }
    let mut present = [false; WINDOW + JOINERS.len()];
    for &slot in entries.iter().flatten() {
        present[slot] = true;
    }
    let mut symbols = [0u8; WINDOW + JOINERS.len()];
    let mut next = 1usize;
    for (slot, used) in present.iter().enumerate() {
        if *used {
            symbols[slot] = u8::try_from(next)
                .map_err(|_| Error::Data(format!("{file} uses more than 255 characters")))?;
            next += 1;
        }
    }
    let mut words: Vec<Vec<u8>> = entries
        .iter()
        .map(|entry| entry.iter().map(|&slot| symbols[slot]).collect())
        .collect();
    words.sort_unstable();
    words.dedup();
    let mut window_symbols = [0u8; WINDOW];
    window_symbols.copy_from_slice(&symbols[..WINDOW]);
    Ok(WordList {
        symbols: window_symbols,
        words,
    })
}
