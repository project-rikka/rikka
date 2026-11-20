use rikka_llm::{Trouble, Window};

#[derive(Debug, Default)]
pub struct Reading {
    nth: u64,
    waiting: Option<u64>,
    windows: Vec<Window>,
    trouble: Option<Trouble>,
}

impl Reading {
    pub(crate) fn next_request(&mut self) -> u64 {
        self.nth += 1;
        self.waiting = Some(self.nth);
        self.trouble = None;
        self.nth
    }

    pub(crate) fn record(&mut self, nth: u64, read: Result<Vec<Window>, Trouble>) -> bool {
        if self.waiting != Some(nth) {
            return false;
        }
        self.waiting = None;
        match read {
            Ok(windows) => self.windows = windows,
            Err(refused) => self.trouble = Some(refused),
        }
        true
    }

    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.waiting.is_some()
    }

    #[must_use]
    pub fn windows(&self) -> &[Window] {
        &self.windows
    }

    #[must_use]
    pub fn error(&self) -> Option<&Trouble> {
        self.trouble.as_ref()
    }
}
