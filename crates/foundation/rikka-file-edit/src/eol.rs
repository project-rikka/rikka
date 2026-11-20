#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Eol {
    #[default]
    Lf,
    Crlf,
}

impl Eol {
    pub(crate) fn detect(text: &str) -> Eol {
        match text
            .match_indices('\n')
            .all(|(at, _)| at > 0 && text.as_bytes()[at - 1] == b'\r')
        {
            true if text.contains('\n') => Eol::Crlf,
            _ => Eol::Lf,
        }
    }
}
