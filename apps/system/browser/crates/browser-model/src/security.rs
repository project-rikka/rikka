#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Security {
    #[default]
    Unknown,
    Insecure,
    Secure,
}

impl Security {
    pub fn of(url: &str) -> Security {
        if url.starts_with("https://") {
            Security::Secure
        } else if url.starts_with("http://") {
            Security::Insecure
        } else {
            Security::Unknown
        }
    }
}
