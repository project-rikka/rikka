use rikka_app::prelude::*;

#[derive(Debug)]
pub(crate) struct SpotReveal<S> {
    wanted: Option<S>,
    due: Option<(S, u64)>,
    requests: u64,
}

impl<S: Copy + Eq> SpotReveal<S> {
    pub(crate) fn new() -> Self {
        Self {
            wanted: None,
            due: None,
            requests: 0,
        }
    }

    pub(crate) fn request(&mut self, spot: S) {
        self.wanted = Some(spot);
        self.due = None;
    }

    pub(crate) fn wanted(&self) -> Option<S> {
        self.wanted
    }

    pub(crate) fn settle(&mut self) {
        if let Some(spot) = self.wanted.take() {
            self.requests += 1;
            self.due = Some((spot, self.requests));
        }
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.wanted.is_none() && self.due.is_none()
    }

    pub(crate) fn clear(&mut self) {
        self.wanted = None;
        self.due = None;
    }

    pub(crate) fn section(&self, spot: S, section: Container) -> Element {
        let due = self
            .due
            .filter(|(due, _)| *due == spot)
            .map(|(_, request)| request);
        section
            .reveal(due.map(|_| Reveal::Start))
            .build()
            .keyed(due)
    }
}
