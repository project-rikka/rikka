#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comment {
    Line(&'static str),
    Block(&'static str, &'static str),
}
