use std::collections::HashMap;
use std::rc::Rc;

use shell_kit::Feed;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Decision {
    #[default]
    Pending,
    Made(Option<String>),
}

#[derive(Clone, Default)]
pub struct ShownOn(Rc<Feed<HashMap<String, Decision>>>);

impl ShownOn {
    #[must_use]
    pub fn on(&self, output: &str) -> Decision {
        self.0
            .with(|shown| shown.get(output).cloned().unwrap_or_default())
    }

    pub(super) fn decide(&self, output: &str, decision: Decision) {
        self.0.alter(|shown| {
            let before = shown.insert(output.to_owned(), decision.clone());
            before != Some(decision)
        });
    }
}

impl std::fmt::Debug for ShownOn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShownOn").finish_non_exhaustive()
    }
}
