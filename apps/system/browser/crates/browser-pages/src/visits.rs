use std::collections::VecDeque;

use browser_store::Transition;

const REMEMBERED: usize = 16;
const RECORDED_SCHEMES: [&str; 3] = ["http", "https", "file"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    Typed,
    Generated,
    Link,
    BackForward,
    Reload,
    Restored,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Arrival {
    pub top_level: bool,
    pub failed: bool,
    pub redirect_source: bool,
    pub redirect_target: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Visited {
    pub url: String,
    pub transition: Transition,
}

#[derive(Debug, Default)]
pub struct VisitTracker {
    pending_cause: Option<Cause>,
    decided: VecDeque<(String, Transition)>,
    leg: Option<Option<Transition>>,
}

impl Cause {
    fn transition(self) -> Option<Transition> {
        match self {
            Cause::Typed => Some(Transition::Typed),
            Cause::Generated => Some(Transition::Generated),
            Cause::Link => Some(Transition::Link),
            Cause::BackForward => Some(Transition::BackForward),
            Cause::Reload => Some(Transition::Reload),
            Cause::Restored => None,
        }
    }
}

impl VisitTracker {
    pub fn set_cause(&mut self, cause: Cause) {
        self.pending_cause = Some(cause);
    }

    pub fn set_cause_if_unset(&mut self, cause: Cause) {
        self.pending_cause.get_or_insert(cause);
    }

    pub fn policy_decided(&mut self, uri: &str, user_gesture: bool) {
        if uri.is_empty() {
            return;
        }
        if self.decided.len() == REMEMBERED {
            self.decided.pop_front();
        }
        let transition = match user_gesture {
            true => Transition::Link,
            false => Transition::Redirect,
        };
        self.decided.push_back((uri.to_owned(), transition));
    }

    pub fn navigation_started(&mut self) {
        self.leg = None;
    }

    pub fn visited(&mut self, url: &str, arrival: Arrival) -> Option<Visited> {
        if !arrival.top_level || arrival.failed {
            return None;
        }
        let transition = match self.leg {
            Some(transition) if arrival.redirect_target || arrival.redirect_source => transition,
            _ => self.begin(url),
        };
        self.leg = arrival.redirect_source.then_some(transition);
        let transition = match arrival.redirect_source {
            true => transition.map(|_| Transition::Redirect),
            false => transition,
        }?;
        is_recorded_scheme(url).then(|| Visited {
            url: url.to_owned(),
            transition,
        })
    }

    fn begin(&mut self, url: &str) -> Option<Transition> {
        match self.pending_cause.take() {
            Some(cause) => cause.transition(),
            None => Some(self.take_decided(url).unwrap_or(Transition::Link)),
        }
    }

    fn take_decided(&mut self, url: &str) -> Option<Transition> {
        let at = self
            .decided
            .iter()
            .rposition(|(candidate, _)| candidate == url)?;
        self.decided.remove(at).map(|(_, transition)| transition)
    }
}

#[must_use]
pub fn is_recorded_scheme(url: &str) -> bool {
    url.split_once(':')
        .is_some_and(|(scheme, _)| RECORDED_SCHEMES.contains(&scheme))
}
