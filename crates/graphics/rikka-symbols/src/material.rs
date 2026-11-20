mod names {
    include!("names/material.rs");
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MaterialSymbol(char);

impl MaterialSymbol {
    pub const fn from_codepoint(codepoint: char) -> Self {
        Self(codepoint)
    }

    pub fn from_name(name: &str) -> Option<Self> {
        names::NAMED
            .binary_search_by(|(listed, _)| (*listed).cmp(name))
            .ok()
            .map(|at| names::NAMED[at].1)
    }

    pub const fn codepoint(self) -> char {
        self.0
    }
}
