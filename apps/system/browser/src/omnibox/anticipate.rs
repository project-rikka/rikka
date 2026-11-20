use rikka_omnibox::{Action, Surface};

use crate::shell::Shell;

impl Shell {
    pub(crate) fn anticipate(&mut self, surface: Surface, index: usize) {
        let origin = self
            .picker(surface)
            .lookup
            .as_ref()
            .and_then(|lookup| lookup.ranked().matches.get(index))
            .and_then(|ranked_match| origin_of(&ranked_match.action));
        let Some(origin) = origin else {
            return;
        };
        if self.picker(surface).anticipated.as_ref() == Some(&origin) {
            return;
        }
        let key = self.model().profile_here(self.id());
        let Some(runtime) = self.browser().running_engine(key) else {
            return;
        };
        runtime.preconnect(origin.clone());
        self.picker_mut(surface).anticipated = Some(origin);
    }
}

fn origin_of<K>(action: &Action<K>) -> Option<String> {
    let url = match action {
        Action::Navigate(url) => url,
        Action::Search(search) => search.url(),
        Action::SwitchToTab(_) | Action::RestoreArchived(_) | Action::RunCommand(_) => {
            return None;
        }
    };
    matches!(url.scheme(), "http" | "https").then(|| url.origin().ascii_serialization())
}
