use std::collections::BTreeSet;

pub(crate) struct Expander<'a> {
    lookup: &'a dyn Fn(&str) -> Option<String>,
    unset: BTreeSet<String>,
}

impl<'a> Expander<'a> {
    pub(crate) fn new(lookup: &'a dyn Fn(&str) -> Option<String>) -> Expander<'a> {
        Expander {
            lookup,
            unset: BTreeSet::new(),
        }
    }

    pub(crate) fn unset(self) -> Vec<String> {
        self.unset.into_iter().collect()
    }

    pub(crate) fn expand(&mut self, text: &str) -> String {
        let mut expanded = String::with_capacity(text.len());
        let mut rest = text;

        while let Some(at) = rest.find("${") {
            expanded.push_str(&rest[..at]);
            let after = &rest[at + 2..];
            match after
                .find('}')
                .and_then(|end| self.reference(&after[..end]).map(|value| (end, value)))
            {
                Some((end, value)) => {
                    expanded.push_str(&value);
                    rest = &after[end + 1..];
                }
                None => {
                    expanded.push_str("${");
                    rest = after;
                }
            }
        }

        expanded.push_str(rest);
        expanded
    }

    fn reference(&mut self, inner: &str) -> Option<String> {
        let (name, fallback) = match inner.split_once(":-") {
            Some((name, fallback)) => (name, Some(fallback)),
            None => (inner, None),
        };
        if !is_name(name) {
            return None;
        }

        let found = (self.lookup)(name).filter(|value| !value.is_empty());
        match (found, fallback) {
            (Some(value), _) => Some(value),
            (None, Some(fallback)) => Some(fallback.to_owned()),
            (None, None) => {
                self.unset.insert(name.to_owned());
                Some(String::new())
            }
        }
    }
}

fn is_name(name: &str) -> bool {
    let mut letters = name.chars();
    letters
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
        && letters.all(|letter| letter == '_' || letter.is_ascii_alphanumeric())
}
