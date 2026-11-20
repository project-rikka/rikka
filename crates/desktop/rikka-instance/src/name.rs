#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name {
    bus: String,
    path: String,
}

impl Name {
    #[must_use]
    pub fn from_app_id(id: &str) -> Option<Name> {
        let bus = if is_bus_name(id) {
            id.to_owned()
        } else {
            rikka_bus_name(id)?
        };
        let path = format!("/{}", bus.replace('.', "/"));
        Some(Name { bus, path })
    }

    #[must_use]
    pub fn bus(&self) -> &str {
        &self.bus
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
}

fn rikka_bus_name(id: &str) -> Option<String> {
    let mut parts: Vec<&str> = id
        .split(['-', '_', '.', ' '])
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() > 1 && parts[0].eq_ignore_ascii_case("rikka") {
        parts.remove(0);
    }
    let element: String = parts.into_iter().map(capitalized).collect();
    let element: String = element
        .chars()
        .filter(|letter| letter.is_ascii_alphanumeric() || *letter == '_')
        .collect();
    if element.is_empty() {
        return None;
    }
    let element = match element.starts_with(|letter: char| letter.is_ascii_digit()) {
        true => format!("_{element}"),
        false => element,
    };
    Some(format!("org.rikka.{element}"))
}

fn capitalized(part: &str) -> String {
    let mut letters = part.chars();
    match letters.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + letters.as_str(),
        None => String::new(),
    }
}

fn is_bus_name(id: &str) -> bool {
    let elements: Vec<&str> = id.split('.').collect();
    id.len() < 255
        && elements.len() > 1
        && elements.iter().all(|element| {
            element.starts_with(|letter: char| letter.is_ascii_alphabetic() || letter == '_')
                && element
                    .chars()
                    .all(|letter| letter.is_ascii_alphanumeric() || letter == '_')
        })
}
