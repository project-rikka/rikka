use rand::Rng as _;
use rand::seq::IndexedRandom as _;

const LETTERS: &str = "abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ";
const DIGITS: &str = "23456789";
const SYMBOLS: &str = "!#$%&()*+-<=>?@[]^_{|}~";

pub const MIN_PASSWORD_LENGTH: usize = 4;
pub const MAX_PASSWORD_LENGTH: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PasswordOptions {
    pub length: usize,
    pub digits: bool,
    pub symbols: bool,
}

impl PasswordOptions {
    #[must_use]
    pub fn new() -> PasswordOptions {
        PasswordOptions {
            length: 20,
            digits: true,
            symbols: true,
        }
    }
}

impl Default for PasswordOptions {
    fn default() -> PasswordOptions {
        PasswordOptions::new()
    }
}

#[must_use]
pub fn generate_password(recipe: &PasswordOptions) -> String {
    let length = recipe
        .length
        .clamp(MIN_PASSWORD_LENGTH, MAX_PASSWORD_LENGTH);
    let mut pools: Vec<Vec<char>> = vec![LETTERS.chars().collect()];
    if recipe.digits {
        pools.push(DIGITS.chars().collect());
    }
    if recipe.symbols {
        pools.push(SYMBOLS.chars().collect());
    }

    let mut rng = rand::rng();
    let mut letters: Vec<char> = Vec::with_capacity(length);

    for pool in &pools {
        if letters.len() < length
            && let Some(letter) = pool.choose(&mut rng)
        {
            letters.push(*letter);
        }
    }

    let all: Vec<char> = pools.iter().flatten().copied().collect();
    while letters.len() < length {
        let at = rng.random_range(0..all.len());
        letters.push(all[at]);
    }

    for at in (1..letters.len()).rev() {
        let other = rng.random_range(0..=at);
        letters.swap(at, other);
    }

    letters.into_iter().collect()
}
