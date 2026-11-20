#[derive(Clone, Debug)]
pub struct Group {
    name: String,
    entries: Vec<(String, Option<String>, String)>,
}

impl Group {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn raw(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(name, locale, _)| name == key && locale.is_none())
            .map(|(_, _, value)| value.as_str())
    }

    #[must_use]
    pub fn string(&self, key: &str) -> Option<String> {
        self.raw(key).map(unescape)
    }

    #[must_use]
    pub fn boolean(&self, key: &str) -> bool {
        self.raw(key) == Some("true")
    }

    #[must_use]
    pub fn localized(&self, key: &str, locales: &[String]) -> Option<String> {
        self.written(key, locales).map(unescape)
    }

    fn written(&self, key: &str, locales: &[String]) -> Option<&str> {
        locales
            .iter()
            .find_map(|candidate| {
                self.entries
                    .iter()
                    .find(|(name, locale, _)| name == key && locale.as_deref() == Some(candidate))
            })
            .map(|(_, _, value)| value.as_str())
            .or_else(|| self.raw(key))
    }

    #[must_use]
    pub fn list(&self, key: &str, locales: &[String]) -> Vec<String> {
        match self.written(key, locales) {
            Some(value) => split_list(value),
            None => Vec::new(),
        }
    }

    pub fn entries(&self) -> impl Iterator<Item = (&str, Option<&str>, &str)> {
        self.entries
            .iter()
            .map(|(key, locale, value)| (key.as_str(), locale.as_deref(), value.as_str()))
    }
}

#[derive(Clone, Debug, Default)]
pub struct KeyFile {
    groups: Vec<Group>,
}

impl KeyFile {
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut groups: Vec<Group> = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                groups.push(Group {
                    name: name.to_owned(),
                    entries: Vec::new(),
                });
                continue;
            }
            let Some(group) = groups.last_mut() else {
                continue;
            };
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, locale) = match key.trim_end().split_once('[') {
                Some((key, locale)) => (key.trim_end(), locale.strip_suffix(']')),
                None => (key.trim_end(), None),
            };
            group.entries.push((
                key.to_owned(),
                locale.map(str::to_owned),
                value.trim_start().to_owned(),
            ));
        }
        Self { groups }
    }

    #[must_use]
    pub fn group(&self, name: &str) -> Option<&Group> {
        self.groups.iter().find(|group| group.name == name)
    }

    pub fn groups(&self) -> impl Iterator<Item = &Group> {
        self.groups.iter()
    }
}

fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn split_list(value: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for ch in value.chars() {
        match ch {
            ';' if escaped => {
                current.pop();
                current.push(';');
                escaped = false;
            }
            _ if escaped => {
                current.push(ch);
                escaped = false;
            }
            '\\' => {
                escaped = true;
                current.push(ch);
            }
            ';' => parts.push(std::mem::take(&mut current)),
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts.retain(|part| !part.is_empty());
    parts.iter().map(|part| unescape(part)).collect()
}
