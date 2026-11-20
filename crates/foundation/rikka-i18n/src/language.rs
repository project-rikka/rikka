use crate::global::global;

#[derive(Clone, Debug)]
pub struct Language {
    tag: String,
    name: String,
}

impl Language {
    #[must_use]
    pub fn tag(&self) -> &str {
        &self.tag
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[must_use]
pub fn languages() -> Vec<Language> {
    global()
        .catalogs()
        .map(|catalog| Language {
            tag: catalog.locale().to_string(),
            name: catalog.display_name().to_owned(),
        })
        .collect()
}
