const INSUFFICIENT_SCOPE: &str = "insufficient_scope";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Challenge {
    params: Vec<(String, String)>,
}

impl Challenge {
    #[must_use]
    pub fn parse(www_authenticate: &str) -> Challenge {
        let mut params = Vec::new();
        let mut rest = www_authenticate;
        while let Some(equals) = rest.find('=') {
            let name = rest[..equals]
                .rsplit(|character: char| character == ',' || character.is_whitespace())
                .next()
                .unwrap_or_default();
            let after = rest[equals + 1..].trim_start();
            let (value, remaining) = match after.strip_prefix('"') {
                Some(quoted) => unquoted(quoted),
                None => {
                    let end = after
                        .find(|character: char| character == ',' || character.is_whitespace())
                        .unwrap_or(after.len());
                    (after[..end].to_owned(), &after[end..])
                }
            };
            if !name.is_empty() {
                params.push((name.to_ascii_lowercase(), value));
            }
            rest = remaining;
        }
        Challenge { params }
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(key, value)| key == name && !value.is_empty())
            .map(|(_, value)| value.as_str())
    }

    #[must_use]
    pub fn resource_metadata(&self) -> Option<&str> {
        self.get("resource_metadata")
    }

    #[must_use]
    pub fn scope(&self) -> Option<&str> {
        self.get("scope")
    }

    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.get("error")
    }

    #[must_use]
    pub fn is_insufficient_scope(&self) -> bool {
        self.error() == Some(INSUFFICIENT_SCOPE)
    }
}

fn unquoted(quoted: &str) -> (String, &str) {
    let mut value = String::new();
    let mut characters = quoted.char_indices();
    while let Some((at, character)) = characters.next() {
        match character {
            '\\' => {
                if let Some((_, escaped)) = characters.next() {
                    value.push(escaped);
                }
            }
            '"' => return (value, &quoted[at + 1..]),
            other => value.push(other),
        }
    }
    (value, "")
}
