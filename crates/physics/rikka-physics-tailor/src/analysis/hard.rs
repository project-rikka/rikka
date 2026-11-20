const HARD: &[&str] = &[
    "金属",
    "金屬",
    "メタル",
    "宝石",
    "寶石",
    "ジュエル",
    "武器",
    "剣",
    "剑",
    "劍",
    "銃",
    "枪",
    "槍",
    "盾",
    "鎧",
    "铠",
    "metal",
    "jewel",
    "jewelry",
    "gem",
    "gems",
    "stone",
    "crystal",
    "weapon",
    "sword",
    "katana",
    "gun",
    "shield",
    "armor",
    "armour",
];

pub(crate) fn is_hard<'a>(names: impl IntoIterator<Item = &'a str>) -> bool {
    names.into_iter().any(|name| {
        let tokens = tokens(name);
        HARD.iter().any(|word| match word.is_ascii() {
            false => name.contains(word),
            true if word.len() >= 4 => tokens.iter().any(|token| token.contains(word)),
            true => tokens.iter().any(|token| token == word),
        })
    })
}

fn tokens(name: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut previous: Option<char> = None;
    for letter in name.chars() {
        let split = previous.is_some_and(|before| {
            !letter.is_ascii_alphabetic()
                || (before.is_ascii_lowercase() && letter.is_ascii_uppercase())
                || !before.is_ascii_alphabetic()
        });
        if split && !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
        if letter.is_ascii_alphabetic() {
            current.push(letter.to_ascii_lowercase());
        }
        previous = Some(letter);
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}
