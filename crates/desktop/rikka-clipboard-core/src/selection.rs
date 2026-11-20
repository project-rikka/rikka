#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Selection {
    #[default]
    Clipboard,
    Primary,
}
