#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("not a .cube or .3dl lookup table")]
    Unrecognized,
    #[error("line {line}: {reason}")]
    Line { line: usize, reason: &'static str },
    #[error("1D lookup tables are not supported")]
    OneDimensional,
    #[error("lattice size {0} is outside 2..=256")]
    Size(usize),
    #[error("the table holds {found} entries, a {size}³ lattice needs {expected}")]
    Entries {
        size: usize,
        expected: usize,
        found: usize,
    },
    #[error("the domain is empty along a channel")]
    Domain,
}
