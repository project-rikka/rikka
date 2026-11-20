#[must_use]
pub fn subsequence(candidate: &str, query: &str) -> bool {
    let mut needle = query
        .chars()
        .filter(|ch| !is_separator(*ch))
        .flat_map(char::to_lowercase)
        .peekable();

    for ch in candidate.chars().flat_map(char::to_lowercase) {
        match needle.peek() {
            Some(next) if *next == ch => {
                needle.next();
            }
            Some(_) => {}
            None => return true,
        }
    }
    needle.peek().is_none()
}

fn is_separator(ch: char) -> bool {
    matches!(ch, ' ' | '/' | '_' | '-' | '.' | '(' | ')')
}
