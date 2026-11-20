use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectionKind {
    Clipboard,
    Primary,
}

impl SelectionKind {
    pub const ALL: [Self; 2] = [Self::Clipboard, Self::Primary];
}

impl fmt::Display for SelectionKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Clipboard => "clipboard",
            Self::Primary => "primary",
        })
    }
}
