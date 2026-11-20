use std::collections::HashMap;

use rikka_xdg::Catalog;

#[derive(Debug)]
pub(crate) struct Applications(HashMap<String, String>);

impl Applications {
    pub(crate) fn scan() -> Self {
        let mut names: HashMap<String, String> = HashMap::new();
        for entry in Catalog::read().listable() {
            let Some(program) = entry.program() else {
                continue;
            };
            names.entry(program).or_insert_with(|| entry.name.clone());
        }
        Self(names)
    }

    pub(crate) fn name(&self, program: &str) -> Option<&str> {
        self.0.get(program).map(String::as_str)
    }
}
