#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Next,
    Previous,
    Up,
    Down,
    First,
    Last,
}

impl Step {
    pub fn repeats(self) -> bool {
        !matches!(self, Step::First | Step::Last)
    }
}
