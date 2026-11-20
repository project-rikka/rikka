use rikka_ui::DragOffer;

pub(crate) fn typed(offer: &DragOffer) -> Option<String> {
    let uris = offer.uris();
    if uris.is_empty() {
        return offer.text().filter(|text| !text.is_empty());
    }
    let mut words: Vec<String> = uris.iter().map(|uri| quoted(&named(uri))).collect();
    words.push(String::new());
    Some(words.join(" "))
}

fn named(uri: &str) -> String {
    rikka_open::uri_to_path(uri).map_or_else(|| uri.to_owned(), |path| path.display().to_string())
}

fn quoted(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"/._-+,:@%=".contains(&byte));
    if plain {
        return word.to_owned();
    }
    format!("'{}'", word.replace('\'', r"'\''"))
}
