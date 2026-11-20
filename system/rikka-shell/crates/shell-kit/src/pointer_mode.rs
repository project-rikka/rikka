use crate::Margin;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum PointerMode {
    #[default]
    Take,
    Ignore,
    Inset(Margin),
    Panes,
}
