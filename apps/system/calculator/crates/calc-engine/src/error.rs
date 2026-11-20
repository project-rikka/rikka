#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    DivisionByZero,
    Undefined,
    InvalidInput,
    Overflow,
}

impl Error {
    pub(crate) fn finite(value: f64) -> Result<f64, Self> {
        if value.is_finite() {
            Ok(value)
        } else {
            Err(Self::Overflow)
        }
    }
}
