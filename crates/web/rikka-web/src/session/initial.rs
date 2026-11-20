const BLANK: &str = "about:blank";

#[derive(Debug, Default)]
pub(crate) enum Initial {
    #[default]
    Blank,
    Asked(Option<String>),
    Past,
}

impl Initial {
    pub(crate) fn ask(&mut self, uri: Option<&str>) {
        if !matches!(self, Initial::Past) {
            *self = Initial::Asked(uri.map(str::to_owned));
        }
    }

    pub(crate) fn allows(&mut self, uri: &str) -> bool {
        let admitted = match self {
            Initial::Past => return true,
            Initial::Asked(Some(url)) if url == BLANK => true,
            Initial::Blank | Initial::Asked(_) => uri != BLANK,
        };
        if admitted {
            *self = Initial::Past;
        }
        admitted
    }

    pub(crate) fn is_past(&self) -> bool {
        matches!(self, Initial::Past)
    }
}
