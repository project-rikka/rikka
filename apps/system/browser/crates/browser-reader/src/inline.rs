use crate::url::Link;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Inline {
    Text(Run),
    Break,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub marks: Marks,
    pub link: Option<Link>,
    pub ruby: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Marks {
    pub emphasis: bool,
    pub strong: bool,
    pub code: bool,
    pub superscript: bool,
    pub subscript: bool,
    pub deleted: bool,
}

impl Run {
    pub fn plain(text: impl Into<String>) -> Run {
        Run {
            text: text.into(),
            marks: Marks::default(),
            link: None,
            ruby: None,
        }
    }
}
