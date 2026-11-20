#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Reveal {
    #[default]
    Nearest,
    Start,
}
