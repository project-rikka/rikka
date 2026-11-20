#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Status {
    Passive,
    #[default]
    Active,
    NeedsAttention,
}

impl Status {
    #[must_use]
    pub(crate) fn named(name: &str) -> Status {
        match name {
            "Passive" => Status::Passive,
            "NeedsAttention" => Status::NeedsAttention,
            _ => Status::Active,
        }
    }

    #[must_use]
    pub fn needs_attention(self) -> bool {
        self == Status::NeedsAttention
    }
}
