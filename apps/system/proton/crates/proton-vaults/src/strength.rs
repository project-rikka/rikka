const VULNERABLE_AT_MOST: f64 = 60.0;
const WEAK_AT_MOST: f64 = 90.0;

const CEILINGS: [f64; 17] = [
    0.0, 2.0, 5.0, 9.0, 16.0, 24.0, 30.0, 45.0, 51.0, 60.0, 69.0, 75.0, 80.0, 86.0, 91.0, 95.0,
    100.0,
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Strength {
    Vulnerable,
    Weak,
    #[default]
    Strong,
}

impl Strength {
    #[must_use]
    pub fn estimate(password: &str) -> Strength {
        Strength::from_score(score(password))
    }

    #[must_use]
    pub fn from_score(score: f64) -> Strength {
        match score {
            score if score <= VULNERABLE_AT_MOST => Strength::Vulnerable,
            score if score < WEAK_AT_MOST => Strength::Weak,
            _ => Strength::Strong,
        }
    }
}

#[must_use]
pub(crate) fn score(password: &str) -> f64 {
    let counted = Counted::count(password);
    let plain = counted.length - counted.other;

    let Some(ceiling) = CEILINGS.get(plain).copied() else {
        return 100.0;
    };

    let mut score = ceiling;
    if score > 0.0 {
        score += counted.spaces as f64;

        if counted.digits == 0 {
            score -= ceiling * 0.05;
        }
        if counted.lowercase == 0 {
            score -= ceiling * 0.1;
        }
        if counted.uppercase == 0 {
            score -= ceiling * 0.1;
        }
        if counted.lowercase >= 1 && counted.uppercase >= 1 {
            score += 1.0;
        }
        match counted.symbols {
            0 => score -= ceiling * 0.2,
            _ => score += 1.0,
        }

        if !counted.is_long_enough() {
            if counted.digits == 0 {
                score -= ceiling * 0.1;
            }
            if counted.uppercase == 0 {
                score -= ceiling * 0.1;
            }
        }

        let length = counted.length as f64;
        score -= ceiling * (counted.repeated as f64 / length / 5.0);
        score -= ceiling * (counted.progressive as f64 / length / 5.0);
        score -= ceiling * (counted.distinct as f64 / length / 10.0);
    }

    (score.clamp(0.0, ceiling) + counted.other as f64 * 20.0).min(100.0)
}

#[derive(Debug, Default)]
struct Counted {
    length: usize,
    spaces: usize,
    digits: usize,
    lowercase: usize,
    uppercase: usize,
    symbols: usize,
    other: usize,
    repeated: usize,
    distinct: usize,
    progressive: usize,
}

impl Counted {
    fn count(password: &str) -> Counted {
        let mut counted = Counted::default();
        let mut counts: Vec<(char, usize)> = Vec::new();
        let mut last: Option<char> = None;
        let mut step: Option<i64> = None;

        for letter in password.chars().filter(|letter| !letter.is_control()) {
            counted.length += 1;
            match letter {
                ' ' => counted.spaces += 1,
                letter if letter.is_ascii_digit() => counted.digits += 1,
                letter if letter.is_ascii_lowercase() => counted.lowercase += 1,
                letter if letter.is_ascii_uppercase() => counted.uppercase += 1,
                letter if letter.is_ascii_graphic() => counted.symbols += 1,
                _ => counted.other += 1,
            }

            match counts.iter_mut().find(|(counted, _)| *counted == letter) {
                Some((_, times)) => *times += 1,
                None => counts.push((letter, 1)),
            }

            if let Some(previous) = last {
                let walked = letter as i64 - previous as i64;
                if walked == 0 {
                    counted.repeated += 1;
                }
                if step == Some(walked) && walked.abs() == 1 {
                    counted.progressive += 1;
                }
                step = Some(walked);
            }
            last = Some(letter);
        }

        counted.distinct = counts.iter().filter(|(_, times)| *times > 1).count();
        counted
    }

    fn is_long_enough(&self) -> bool {
        match self.length {
            length if length < 13 => false,
            length if length < 20 => self.symbols > 0,
            _ => true,
        }
    }
}
