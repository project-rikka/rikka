use crate::operator::Operator;
use crate::unary::Unary;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Digit(u8),
    Point,
    Operator(Operator),
    Unary(Unary),
    Percent,
    Backspace,
    ClearEntry,
    Clear,
    Equals,
}
