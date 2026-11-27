use rand::Rng;
use rand::seq::SliceRandom;

const LOWER: &[u8] = b"abcdefghijkmnpqrstuvwxyz";
const UPPER: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";
const DIGITS: &[u8] = b"23456789";
const SYMBOLS: &[u8] = b"-_.:!";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recipe {
    pub length: usize,
    pub digits: bool,
    pub symbols: bool,
}

impl Recipe {
    pub const SHORTEST: usize = 8;
    pub const LONGEST: usize = 64;

    #[must_use]
    pub fn generate(self) -> String {
        let mut rng = rand::rng();
        let classes: Vec<&[u8]> = [
            Some(LOWER),
            Some(UPPER),
            self.digits.then_some(DIGITS),
            self.symbols.then_some(SYMBOLS),
        ]
        .into_iter()
        .flatten()
        .collect();
        let alphabet: Vec<u8> = classes.concat();
        let length = self.length.clamp(Recipe::SHORTEST, Recipe::LONGEST);
        let mut password: Vec<u8> = classes
            .iter()
            .map(|class| class[rng.random_range(0..class.len())])
            .collect();
        while password.len() < length {
            password.push(alphabet[rng.random_range(0..alphabet.len())]);
        }
        password.shuffle(&mut rng);
        password.into_iter().map(char::from).collect()
    }
}

impl Default for Recipe {
    fn default() -> Recipe {
        Recipe {
            length: 15,
            digits: true,
            symbols: true,
        }
    }
}
