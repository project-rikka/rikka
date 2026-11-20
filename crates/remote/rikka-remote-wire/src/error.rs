#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    #[error("empty message")]
    Empty,
    #[error("unknown type {0:#04x}")]
    UnknownType(u8),
    #[error("type {0:#04x} is truncated")]
    Truncated(u8),
    #[error("type {0:#04x} has trailing bytes")]
    Trailing(u8),
    #[error("type {0:#04x} carries invalid UTF-8")]
    InvalidText(u8),
    #[error("type {0:#04x} carries a non-finite number")]
    NonFinite(u8),
    #[error("invalid {field} {value}")]
    InvalidValue { field: &'static str, value: u64 },
    #[error("a message of {0} bytes is longer than allowed")]
    TooLong(u64),
}
