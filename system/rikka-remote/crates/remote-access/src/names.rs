const LONGEST_NAME: usize = 200;
const FALLBACK_NAME: &str = "file";

#[must_use]
pub fn clean_file_name(requested: &str) -> String {
    let last = requested
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .chars()
        .filter(|character| !character.is_control())
        .collect::<String>();
    let trimmed = last.trim().trim_start_matches('.').trim();
    let mut name = String::new();
    for character in trimmed.chars() {
        if name.len() + character.len_utf8() > LONGEST_NAME {
            break;
        }
        name.push(character);
    }
    match name.is_empty() {
        true => FALLBACK_NAME.to_owned(),
        false => name,
    }
}
