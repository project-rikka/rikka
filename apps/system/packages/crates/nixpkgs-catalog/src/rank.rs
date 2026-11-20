use std::cmp::Reverse;

const EXACT: u32 = 900;
const PREFIX: u32 = 300;
const WITHIN: u32 = 90;

const NAMED: u32 = 6;
const OUT_OF: u32 = 10;

pub(crate) struct Fields<'a> {
    pub(crate) attribute: &'a str,
    pub(crate) name: &'a str,
    pub(crate) description: &'a str,
}

pub(crate) type Order = (Reverse<u32>, Reverse<u32>, usize, usize, u32);

pub(crate) fn order(words: &[String], fields: &Fields<'_>, id: u32) -> Order {
    let attribute = fields.attribute;
    let leaf = attribute.rsplit('.').next().unwrap_or(attribute);
    let claim: u32 = words
        .iter()
        .map(|word| {
            within(leaf, word).max(within(attribute, word))
                + within(fields.name, word) * NAMED / OUT_OF
        })
        .sum();
    let mentioned = match claim {
        0 => words
            .iter()
            .map(|word| within(fields.description, word))
            .sum(),
        _ => 0,
    };
    (
        Reverse(claim),
        Reverse(mentioned),
        attribute.matches('.').count(),
        attribute.len(),
        id,
    )
}

fn within(text: &str, word: &str) -> u32 {
    if text == word {
        return EXACT;
    }
    if text.starts_with(word) {
        return PREFIX;
    }
    if text.contains(word) {
        return WITHIN;
    }
    0
}
