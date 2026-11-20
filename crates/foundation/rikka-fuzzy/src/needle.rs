use crate::haystack::{Haystack, fold};
use crate::measure::{is_subsequence, measure};
use crate::score::Score;

#[derive(Clone, Debug, Default)]
pub struct Needle {
    atoms: Vec<Atom>,
}

#[derive(Clone, Debug)]
struct Atom {
    letters: Box<[char]>,
    exact: bool,
}

impl Needle {
    #[must_use]
    pub fn new(text: &str) -> Needle {
        Needle::from_atoms(text, |_| false)
    }

    #[must_use]
    pub fn with_smart_case(text: &str) -> Needle {
        Needle::from_atoms(text, |word| word.chars().any(char::is_uppercase))
    }

    fn from_atoms(text: &str, exact: impl Fn(&str) -> bool) -> Needle {
        let atoms = text
            .split_whitespace()
            .map(|word| {
                let exact = exact(word);
                let letters = match exact {
                    true => word.chars().collect(),
                    false => word.chars().map(fold).collect(),
                };
                Atom { letters, exact }
            })
            .collect();
        Needle { atoms }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.atoms.is_empty()
    }

    #[must_use]
    pub fn score(&self, candidate: &str) -> Option<Score> {
        if self.atoms.is_empty() {
            return Some(Score::NEUTRAL);
        }
        self.find(&Haystack::new(candidate))
    }

    #[must_use]
    pub fn find(&self, haystack: &Haystack) -> Option<Score> {
        self.matched(haystack, None)
    }

    #[must_use]
    pub fn positions(&self, haystack: &Haystack, positions: &mut Vec<usize>) -> Option<Score> {
        positions.clear();
        let score = self.matched(haystack, Some(positions));
        match score {
            Some(_) => {
                positions.sort_unstable();
                positions.dedup();
            }
            None => positions.clear(),
        }
        score
    }

    fn matched(
        &self,
        haystack: &Haystack,
        mut positions: Option<&mut Vec<usize>>,
    ) -> Option<Score> {
        let mut total = Score::NEUTRAL;
        for atom in &self.atoms {
            let letters = match atom.exact {
                true => &haystack.exact,
                false => &haystack.folded,
            };
            if !is_subsequence(&atom.letters, letters) {
                return None;
            }
            let score = measure(
                &atom.letters,
                letters,
                &haystack.bonus,
                positions.as_deref_mut(),
            );
            total = total + Score::new(score);
        }
        Some(total)
    }
}
