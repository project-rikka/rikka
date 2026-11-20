use crate::error::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operator {
    Add,
    Subtract,
    Multiply,
    Divide,
}

impl Operator {
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Subtract => "−",
            Self::Multiply => "×",
            Self::Divide => "÷",
        }
    }

    pub(crate) const fn percent_is_relative(self) -> bool {
        matches!(self, Self::Add | Self::Subtract)
    }

    pub(crate) fn apply(self, left: f64, right: f64) -> Result<f64, Error> {
        match self {
            Self::Add => Error::finite(left + right),
            Self::Subtract => Error::finite(left - right),
            Self::Multiply => Error::finite(left * right),
            Self::Divide => divide(left, right),
        }
    }
}

fn divide(left: f64, right: f64) -> Result<f64, Error> {
    if right != 0.0 {
        return Error::finite(left / right);
    }
    Err(if left == 0.0 {
        Error::Undefined
    } else {
        Error::DivisionByZero
    })
}
