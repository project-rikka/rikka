use rikka_sync::Mutex;
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub struct Deferred {
    tools: Arc<Mutex<DeferredState>>,
}

#[derive(Debug, Default)]
struct DeferredState {
    holding: BTreeSet<String>,
    revealed: BTreeSet<String>,
    introduced: BTreeSet<String>,
}

impl Deferred {
    #[must_use]
    pub fn new() -> Deferred {
        Deferred::default()
    }

    pub fn defer(&self, name: impl Into<String>) {
        self.tools.lock().holding.insert(name.into());
    }

    pub fn reveal(&self, name: &str) {
        self.tools.lock().revealed.insert(name.to_owned());
    }

    #[must_use]
    pub fn is_hidden(&self, name: &str) -> bool {
        let tools = self.tools.lock();
        tools.holding.contains(name) && !tools.revealed.contains(name)
    }

    #[must_use]
    pub fn is_revealed(&self, name: &str) -> bool {
        self.tools.lock().revealed.contains(name)
    }

    #[must_use]
    pub fn is_introduced(&self, kit: &str) -> bool {
        self.tools.lock().introduced.contains(kit)
    }

    #[must_use]
    pub fn introduce(&self, kit: &str) -> bool {
        self.tools.lock().introduced.insert(kit.to_owned())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tools.lock().holding.is_empty()
    }
}
