use rikka_text_unicode::{nfkc, nfkd};

#[must_use]
pub fn fold(text: &str) -> String {
    let stripped: String = nfkd(text)
        .chars()
        .filter(|character| !('\u{0300}'..='\u{036f}').contains(character))
        .flat_map(char::to_lowercase)
        .collect();
    let mut folded = String::with_capacity(stripped.len());
    for word in nfkc(&stripped).split_whitespace() {
        if !folded.is_empty() {
            folded.push(' ');
        }
        folded.push_str(word);
    }
    folded
}
