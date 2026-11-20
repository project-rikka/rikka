const OPERATORS: [char; 8] = ['+', '*', '/', '^', '%', '(', '=', '√'];

const CONVERSIONS: [&str; 3] = ["to", "in", "as"];

pub(super) fn evaluating(text: &str) -> bool {
    if !text.chars().any(|ch| ch.is_ascii_digit()) {
        return false;
    }
    if text.chars().any(|ch| OPERATORS.contains(&ch)) {
        return true;
    }
    if text
        .split_whitespace()
        .any(|word| word == "-" || word == "−")
    {
        return true;
    }
    text.split_whitespace().any(|word| {
        CONVERSIONS
            .iter()
            .any(|known| word.eq_ignore_ascii_case(known))
    })
}
