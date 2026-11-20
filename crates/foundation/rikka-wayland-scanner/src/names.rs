const KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "do", "dyn",
    "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl", "in", "let",
    "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref", "return",
    "static", "struct", "trait", "true", "try", "type", "typeof", "unsafe", "unsized", "use",
    "virtual", "where", "while", "yield",
];

const UNESCAPABLE: &[&str] = &["self", "Self", "super", "crate"];

pub(crate) fn camel(name: &str) -> String {
    let joined: String = name
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            match characters.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + characters.as_str(),
                None => String::new(),
            }
        })
        .collect();
    leading_digit(joined)
}

pub(crate) fn screaming_snake(name: &str) -> String {
    leading_digit(name.to_ascii_uppercase())
}

pub(crate) fn snake(name: &str) -> String {
    if UNESCAPABLE.contains(&name) {
        return format!("{name}_");
    }
    if KEYWORDS.contains(&name) {
        return format!("r#{name}");
    }
    leading_digit(name.to_owned())
}

fn leading_digit(name: String) -> String {
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{name}")
    } else {
        name
    }
}
