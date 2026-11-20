#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Travel {
    Left,
    Right,
    WordLeft,
    WordRight,
    LineStart,
    LineEnd,
    Start,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Erase {
    Back,
    Forward,
    WordBack,
    WordForward,
}
