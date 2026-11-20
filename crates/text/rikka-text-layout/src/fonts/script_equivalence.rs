use rikka_sync::RwLock;

use rikka_text_unicode::Script;

use crate::style::Caps;

type Pair = (Script, Script, bool, Caps);

#[derive(Debug, Default)]
pub(crate) struct ScriptEquivalence {
    known: RwLock<Vec<(Pair, bool)>>,
}

impl ScriptEquivalence {
    pub(crate) fn get_or_insert(&self, pair: Pair, decide: impl FnOnce() -> bool) -> bool {
        let found = self
            .known
            .read()
            .iter()
            .find(|(known, _)| *known == pair)
            .map(|&(_, equivalent)| equivalent);
        if let Some(equivalent) = found {
            return equivalent;
        }
        let equivalent = decide();
        self.known.write().push((pair, equivalent));
        equivalent
    }
}
