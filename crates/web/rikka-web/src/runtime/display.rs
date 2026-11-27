use std::sync::Weak;
use std::thread::JoinHandle;

use rikka_nested::{Event, Events};

use crate::runtime::core::Core;

pub(crate) fn follow(events: Events, core: Weak<Core>) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("rikka-web-display".into())
        .spawn(move || {
            for event in events {
                let Some(core) = core.upgrade() else {
                    return;
                };
                core.displayed(event);
            }
        })
}

impl Core {
    fn displayed(&self, event: Event) {
        match event {
            Event::Attached { view } => self.attached(view),
            Event::Unclaimed { toplevel, title } => self.unclaimed(toplevel, title),
            Event::UnclaimedClosed { toplevel } => self.claimed(toplevel),
            Event::ActivationRequested { view } => {
                let id = self.state().views.get(&view).copied();
                if let Some(id) = id {
                    self.activation_requested(id);
                }
            }
            Event::ClientRefused { uid } => {
                tracing::warn!(uid, "the engine display refused a foreign client");
            }
            Event::ClientGone { pid, reason } => {
                tracing::debug!(pid, %reason, "a Chrome process left the engine display");
            }
            _ => {}
        }
    }
}
