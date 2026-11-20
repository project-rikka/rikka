const MATCH_SLASH: f32 = 0.9;
const MATCH_WORD: f32 = 0.8;
const MATCH_CAPITAL: f32 = 0.7;
const MATCH_DOT: f32 = 0.6;

#[derive(Clone, Debug)]
pub struct Haystack {
    pub(crate) exact: Box<[char]>,
    pub(crate) folded: Box<[char]>,
    pub(crate) bonus: Box<[f32]>,
}

impl Haystack {
    #[must_use]
    pub fn new(text: &str) -> Haystack {
        let exact: Box<[char]> = text.chars().collect();
        let folded = exact.iter().map(|letter| fold(*letter)).collect();
        let mut previous = '/';
        let bonus = exact
            .iter()
            .map(|letter| {
                let bonus = bonus_of(previous, *letter);
                previous = *letter;
                bonus
            })
            .collect();
        Haystack {
            exact,
            folded,
            bonus,
        }
    }
}

pub(crate) fn fold(letter: char) -> char {
    letter.to_lowercase().next().unwrap_or(letter)
}

fn bonus_of(previous: char, letter: char) -> f32 {
    match previous {
        '/' => MATCH_SLASH,
        '-' | '_' | ' ' => MATCH_WORD,
        '.' => MATCH_DOT,
        lower if lower.is_lowercase() && letter.is_uppercase() => MATCH_CAPITAL,
        _ => 0.0,
    }
}
