#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("hold {0} has a non-finite point")]
    NonFinitePoint(usize),
    #[error("hold {0} has a non-positive reach")]
    NonPositiveReach(usize),
    #[error("hold {0} ends before it starts")]
    EndsBeforeStart(usize),
    #[error("hold {0} exceeds the maximum ease")]
    EaseTooLong(usize),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
