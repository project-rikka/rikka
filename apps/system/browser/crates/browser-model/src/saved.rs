use crate::Favicon;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Saved {
    pub url: String,
    pub title: String,
    pub favicon: Option<Favicon>,
}

impl Saved {
    pub fn new(url: impl Into<String>, title: impl Into<String>) -> Saved {
        Saved {
            url: url.into(),
            title: title.into(),
            favicon: None,
        }
    }
}
