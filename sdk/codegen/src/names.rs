pub(crate) fn pascal(name: &str) -> String {
    pascal_with_acronyms(name, &[])
}

pub(crate) fn pascal_with_acronyms(name: &str, acronyms: &[&str]) -> String {
    name.split('_')
        .map(|word| {
            if acronyms.contains(&word) {
                return word.to_ascii_uppercase();
            }
            let mut characters = word.chars();
            match characters.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + characters.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

pub(crate) fn screaming(name: &str) -> String {
    name.to_ascii_uppercase()
}

pub(crate) fn spaced(name: &str) -> String {
    name.replace('_', " ")
}
