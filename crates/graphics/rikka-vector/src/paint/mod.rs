pub(crate) mod color_line;
pub(crate) mod gradient;
pub(crate) mod pattern;
pub(crate) mod shader;

use rikka_types::Color;

use crate::paint::gradient::Gradient;
use crate::paint::pattern::Pattern;

#[derive(Clone, Copy, Debug)]
pub enum Paint<'a> {
    Solid(Color),
    Gradient(Gradient<'a>),
    Pattern(Pattern<'a>),
}
