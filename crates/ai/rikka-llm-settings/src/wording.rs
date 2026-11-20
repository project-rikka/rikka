use rikka_llm::Price;

const ZERO: &str = "0";

pub(crate) fn price(price: Price) -> String {
    format!("${} / ${}", rate(price.input), rate(price.output))
}

pub(crate) fn masked(key: &str) -> String {
    const HEAD: usize = 7;
    const TAIL: usize = 4;

    let letters = key.chars().count();
    if letters == 0 {
        return String::new();
    }
    if letters <= HEAD + TAIL {
        return "•".repeat(letters);
    }

    let head = key.char_indices().nth(HEAD).map_or(key.len(), |(at, _)| at);
    let tail = key
        .char_indices()
        .nth(letters - TAIL)
        .map_or(key.len(), |(at, _)| at);
    format!("{}…{}", &key[..head], &key[tail..])
}

fn rate(dollars: f64) -> String {
    let formatted = match dollars {
        rate if rate <= 0.0 => return ZERO.to_owned(),
        rate if rate < 0.01 => format!("{rate:.4}"),
        rate if rate < 1.0 => format!("{rate:.3}"),
        rate => format!("{rate:.2}"),
    };
    match formatted.trim_end_matches('0').trim_end_matches('.') {
        "" => ZERO.to_owned(),
        trimmed => trimmed.to_owned(),
    }
}
