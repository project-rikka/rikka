#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Combine {
    #[default]
    Replace,
    Union,
    Exclude,
    Intersect,
    Xor,
}

impl Combine {
    pub(crate) fn cover(self, existing_coverage: u8, added: u8) -> u8 {
        match self {
            Combine::Replace => added,
            Combine::Union => existing_coverage.max(added),
            Combine::Exclude => existing_coverage.min(u8::MAX - added),
            Combine::Intersect => existing_coverage.min(added),
            Combine::Xor => existing_coverage
                .min(u8::MAX - added)
                .max(added.min(u8::MAX - existing_coverage)),
        }
    }
}
