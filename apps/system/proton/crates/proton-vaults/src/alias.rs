const CAPACITY: [char; 3] = ['_', '-', '.'];

#[must_use]
pub fn alias_prefix(title: &str) -> String {
    let mut prefix = String::new();
    for letter in title.trim().to_lowercase().chars() {
        let letter = match letter {
            ' ' => '-',
            letter => letter,
        };
        if !(letter.is_ascii_alphanumeric() || CAPACITY.contains(&letter)) {
            continue;
        }
        if letter == '.' && (prefix.is_empty() || prefix.ends_with('.')) {
            continue;
        }
        prefix.push(letter);
    }
    prefix
}
