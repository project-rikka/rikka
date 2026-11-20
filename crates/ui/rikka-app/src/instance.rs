use std::sync::{Arc, Mutex};

use rikka_instance::{LaunchRequest, Sink, TAG_ENV};

use crate::tasks::{LoopEvent, LoopProxy};
use crate::window::{IntoWindows, Opening};

#[must_use]
pub struct Instance {
    pub(crate) policy: Policy,
}

pub(crate) enum Policy {
    Single,
    Windows(Box<dyn Fn(LaunchRequest) -> Vec<Opening>>),
}

impl Instance {
    pub fn single() -> Instance {
        Instance {
            policy: Policy::Single,
        }
    }

    pub fn windows<W: IntoWindows>(make: impl Fn(LaunchRequest) -> W + 'static) -> Instance {
        Instance {
            policy: Policy::Windows(Box::new(move |open| make(open).into_windows())),
        }
    }
}

impl std::fmt::Debug for Instance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let policy = match self.policy {
            Policy::Single => "single",
            Policy::Windows(_) => "windows",
        };
        f.debug_struct("Instance").field("policy", &policy).finish()
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Inbox(Arc<Mutex<InboxState>>);

#[derive(Debug, Default)]
enum InboxState {
    #[default]
    Early,
    Waiting(Vec<LaunchRequest>),
    Running(LoopProxy),
}

impl Inbox {
    pub(crate) fn sink(&self) -> Sink {
        let state = Arc::clone(&self.0);
        Arc::new(move |open: LaunchRequest| {
            let Ok(mut state) = state.lock() else {
                return;
            };
            match &mut *state {
                InboxState::Running(proxy) => {
                    let _ = proxy.send_event(LoopEvent::Opened(Box::new(open)));
                }
                InboxState::Waiting(waiting) => waiting.push(open),
                InboxState::Early => *state = InboxState::Waiting(vec![open]),
            }
        })
    }

    pub(crate) fn install(&self, proxy: LoopProxy) {
        let Ok(mut state) = self.0.lock() else {
            return;
        };
        let waiting = match std::mem::replace(&mut *state, InboxState::Running(proxy.clone())) {
            InboxState::Waiting(waiting) => waiting,
            InboxState::Early | InboxState::Running(_) => Vec::new(),
        };
        for open in waiting {
            let _ = proxy.send_event(LoopEvent::Opened(Box::new(open)));
        }
    }
}

pub(crate) fn identity(name: String) -> String {
    match asked_tag() {
        Some(tag) => tagged(&name, &tag).unwrap_or(name),
        None => name,
    }
}

fn asked_tag() -> Option<String> {
    let requested_tag = std::env::var(TAG_ENV).ok()?;
    unsafe {
        std::env::remove_var(TAG_ENV);
    }
    Some(requested_tag)
}

fn tagged(name: &str, requested_tag: &str) -> Option<String> {
    let tag: String = requested_tag
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|letter| letter.to_ascii_lowercase())
        .collect();
    (!tag.is_empty()).then(|| format!("{name}-{tag}"))
}
