use crate::grammar::Grammar;

#[derive(Clone, Copy, Debug)]
pub struct Pair {
    pub opens: char,
    pub closes: char,
}

const PAIRS: [Pair; 3] = [
    Pair {
        opens: '{',
        closes: '}',
    },
    Pair {
        opens: '(',
        closes: ')',
    },
    Pair {
        opens: '[',
        closes: ']',
    },
];

#[derive(Clone, Copy, Debug, Default)]
pub struct Depths([isize; 3]);

impl Depths {
    pub fn count(&mut self, letter: char) {
        for (depth, pair) in self.0.iter_mut().zip(&PAIRS) {
            *depth += isize::from(letter == pair.opens) - isize::from(letter == pair.closes);
        }
    }

    #[must_use]
    pub fn balanced(self) -> bool {
        self.0.iter().all(|&depth| depth == 0)
    }

    pub fn unbalanced(self) -> impl Iterator<Item = (Pair, isize)> {
        PAIRS
            .into_iter()
            .zip(self.0)
            .filter(|(_, depth)| *depth != 0)
    }
}

impl Grammar {
    #[must_use]
    pub fn depths(self, text: &str) -> Option<Depths> {
        let tree = self.parse(text)?;
        let mut depths = Depths::default();
        let mut waiting = vec![tree.root_node()];
        while let Some(node) = waiting.pop() {
            match node.child_count() {
                0 if !node.is_named() && !node.is_missing() => {
                    node.kind().chars().for_each(|letter| depths.count(letter));
                }
                0 => {}
                _ => waiting.extend(node.children(&mut node.walk())),
            }
        }
        Some(depths)
    }
}
