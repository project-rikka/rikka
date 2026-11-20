#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Service {
    pub slug: &'static str,
    pub name: &'static str,
    pub mark: Mark,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mark {
    Brand(char),
    Symbol(char),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Streaming,
    Local,
}

impl Service {
    #[must_use]
    pub fn is_local(&self) -> bool {
        self.kind == Kind::Local
    }

    pub(crate) fn has_valid_slug(&self) -> bool {
        let slug = self.slug.as_bytes();
        !slug.is_empty()
            && slug.first() != Some(&b'-')
            && slug.last() != Some(&b'-')
            && !self.slug.contains("--")
            && slug
                .iter()
                .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'-'))
    }
}
