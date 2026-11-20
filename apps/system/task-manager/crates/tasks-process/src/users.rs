use std::collections::HashMap;

use rikka_dirs::passwd;

#[derive(Debug, Default)]
pub(crate) struct Users(HashMap<u32, String>);

impl Users {
    pub(crate) fn name(&mut self, uid: u32) -> String {
        self.0
            .entry(uid)
            .or_insert_with(|| {
                passwd::lookup(uid)
                    .map(|entry| entry.name)
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| uid.to_string())
            })
            .clone()
    }
}
