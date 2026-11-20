use crate::source::{DEFAULT_LAYOUT, InputSource};

const GROUPS: usize = 4;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Keymap {
    pub layout: String,
    pub variant: String,
    pub model: String,
    pub options: Option<String>,
}

impl Keymap {
    #[must_use]
    pub fn from_sources(
        sources: &[InputSource],
        model: impl Into<String>,
        options: Option<String>,
    ) -> Keymap {
        let used = &sources[..sources.len().min(GROUPS)];
        let (layout, variant) = match used.is_empty() {
            true => (DEFAULT_LAYOUT.to_owned(), String::new()),
            false => (
                joined(used, |source| &source.layout),
                joined(used, |source| &source.variant),
            ),
        };
        Keymap {
            layout,
            variant,
            model: model.into(),
            options: options.filter(|options| !options.is_empty()),
        }
    }

    #[must_use]
    pub fn describe(&self) -> String {
        let mut variants = self.variant.split(',');
        self.layout
            .split(',')
            .map(|layout| match variants.next().unwrap_or_default() {
                "" => layout.to_owned(),
                variant => format!("{layout}({variant})"),
            })
            .collect::<Vec<String>>()
            .join(",")
    }
}

fn joined(sources: &[InputSource], field: impl Fn(&InputSource) -> &String) -> String {
    sources
        .iter()
        .map(|source| field(source).as_str())
        .collect::<Vec<&str>>()
        .join(",")
}
