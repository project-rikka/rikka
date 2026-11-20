#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid selector `{selector}`: {expected} expected at byte {at}")]
    Selector {
        selector: String,
        at: usize,
        expected: &'static str,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
