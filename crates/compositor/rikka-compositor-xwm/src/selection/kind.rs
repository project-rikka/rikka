use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectionKind {
    Clipboard,
    Primary,
    Drag,
}

impl fmt::Display for SelectionKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Clipboard => "clipboard",
            Self::Primary => "primary",
            Self::Drag => "dragged",
        })
    }
}
