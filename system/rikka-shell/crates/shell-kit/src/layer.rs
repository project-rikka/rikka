#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Layer {
    Background,
    Bottom,
    #[default]
    Top,
    Overlay,
}
