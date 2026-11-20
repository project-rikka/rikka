use std::time::{Duration, Instant};

pub const FOCUS_DWELL: Duration = Duration::from_millis(750);

#[derive(Debug)]
pub struct Recency<W> {
    focused: Option<FocusEntry<W>>,
    before: Vec<W>,
}

#[derive(Debug, Clone, Copy)]
struct FocusEntry<W> {
    window: W,
    since: Instant,
    glance: bool,
}

impl<W> Default for Recency<W> {
    fn default() -> Self {
        Self {
            focused: None,
            before: Vec::new(),
        }
    }
}

impl<W: Copy + PartialEq> Recency<W> {
    pub fn focused(&mut self, window: Option<W>, now: Instant) {
        if self.focused.map(|focused| focused.window) == window {
            return;
        }
        if let Some(previous) = self.focused.take()
            && (!previous.glance || now.duration_since(previous.since) >= FOCUS_DWELL)
        {
            self.before.retain(|recent| *recent != previous.window);
            self.before.insert(0, previous.window);
        }
        self.focused = window.map(|window| FocusEntry {
            window,
            since: now,
            glance: false,
        });
    }

    pub fn focused_by_pointer(&mut self, window: W) {
        if let Some(focused) = &mut self.focused
            && focused.window == window
        {
            focused.glance = true;
        }
    }

    pub fn forget(&mut self, window: W) {
        self.before.retain(|recent| *recent != window);
        if self.focused.is_some_and(|focused| focused.window == window) {
            self.focused = None;
        }
    }

    pub fn current(&self) -> Option<W> {
        self.focused.map(|focused| focused.window)
    }

    pub fn order(&self) -> impl Iterator<Item = W> + '_ {
        let current = self.current();
        current.into_iter().chain(
            self.before
                .iter()
                .copied()
                .filter(move |recent| Some(*recent) != current),
        )
    }
}
