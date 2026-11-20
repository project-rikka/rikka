use crate::error::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unary {
    Reciprocal,
    Square,
    SquareRoot,
    Negate,
}

impl Unary {
    pub(crate) const fn function(self) -> &'static str {
        match self {
            Self::Reciprocal => "1/",
            Self::Square => "sqr",
            Self::SquareRoot => "√",
            Self::Negate => "negate",
        }
    }

    pub(crate) fn apply(self, value: f64) -> Result<f64, Error> {
        match self {
            Self::Reciprocal if value == 0.0 => Err(Error::DivisionByZero),
            Self::Reciprocal => Error::finite(1.0 / value),
            Self::Square => Error::finite(value * value),
            Self::SquareRoot if value < 0.0 => Err(Error::InvalidInput),
            Self::SquareRoot => Ok(value.sqrt()),
            Self::Negate => Ok(-value),
        }
    }
}
