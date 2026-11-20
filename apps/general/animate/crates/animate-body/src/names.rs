use std::collections::HashMap;
const HALF_KANA: &str = "ヲァィゥェォャュョッーアイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワン";

const VOICED: &str = "カキクケコサシスセソタチツテトハヒフヘホ";

const SEMI_VOICED: &str = "ハヒフヘホ";

#[must_use]
pub(crate) fn normalized(name: &str) -> String {
    let mut normalized = String::with_capacity(name.len());
    for letter in name.trim().chars() {
        let code = u32::from(letter);
        let letter = match code {
            0xFF01..=0xFF5E => char::from_u32(code - 0xFEE0).unwrap_or(letter),
            0x3000 => ' ',
            0xFF66..=0xFF9D => HALF_KANA
                .chars()
                .nth((code - 0xFF66) as usize)
                .unwrap_or(letter),
            0xFF9E | 0xFF9F => {
                let marked = normalized
                    .pop()
                    .and_then(|last| voiced(last, code == 0xFF9F));
                match marked {
                    Some(marked) => marked,
                    None => continue,
                }
            }
            _ => letter,
        };
        normalized.push(letter.to_ascii_lowercase());
    }
    normalized
}

fn voiced(letter: char, semi: bool) -> Option<char> {
    let step = match (semi, letter) {
        (false, 'ウ') => return Some('ヴ'),
        (false, _) if VOICED.contains(letter) => 1,
        (true, _) if SEMI_VOICED.contains(letter) => 2,
        _ => return Some(letter),
    };
    char::from_u32(u32::from(letter) + step)
}

pub(crate) struct Names {
    first: HashMap<String, usize>,
}

impl Names {
    pub(crate) fn new<'a>(names: impl Iterator<Item = &'a str>) -> Self {
        let mut first = HashMap::new();
        for (index, name) in names.enumerate() {
            first.entry(normalized(name)).or_insert(index);
        }
        Self { first }
    }

    pub(crate) fn find(&self, candidates: &[String]) -> Option<usize> {
        candidates
            .iter()
            .find_map(|candidate| self.first.get(&normalized(candidate)).copied())
    }
}
