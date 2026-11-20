use rikka_compositor_tiling::Edges;

use crate::button::Button;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Title,
    Button(Button),
    Edge(Edges),
}
