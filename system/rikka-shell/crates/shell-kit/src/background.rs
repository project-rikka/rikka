use rikka_types::Color;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum Background {
    #[default]
    Transparent,
    Solid(Color),
}
