pub(crate) fn words(arguments: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;

    for letter in arguments.chars() {
        match quote {
            Some(closing) if letter == closing => quote = None,
            Some(_) => word.push(letter),
            None if matches!(letter, '"' | '\'') => {
                quote = Some(letter);
                started = true;
            }
            None if letter.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            None => {
                word.push(letter);
                started = true;
            }
        }
    }

    if started {
        words.push(word);
    }
    words
}
