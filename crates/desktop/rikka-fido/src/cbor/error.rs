#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("truncated CBOR item")]
    Truncated,

    #[error("indefinite-length CBOR items are not allowed")]
    Indefinite,

    #[error("CBOR item header {0:#04x} is malformed")]
    Malformed(u8),

    #[error("CBOR text string is not UTF-8")]
    Utf8,

    #[error("duplicate key in CBOR map")]
    DuplicateKey,

    #[error("CBOR items nest deeper than {0} levels")]
    TooDeep(usize),

    #[error("{0} trailing bytes after CBOR item")]
    Trailing(usize),

    #[error("CBOR simple value {0} is not supported")]
    Simple(u8),
}
