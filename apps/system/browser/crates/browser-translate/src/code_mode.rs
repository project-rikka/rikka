use rikka_types::Choice;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CodeMode {
    #[default]
    Comments,
    Skip,
}

impl Choice for CodeMode {
    const ALL: &'static [CodeMode] = &[CodeMode::Comments, CodeMode::Skip];

    fn slug(self) -> &'static str {
        match self {
            CodeMode::Comments => "comments",
            CodeMode::Skip => "skip",
        }
    }
}
