use rikka_types::Color;

use super::glass::Glass;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Decor {
    pub(crate) glass: Glass,
    pub(crate) edge: Color,
}
