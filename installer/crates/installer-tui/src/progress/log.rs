use std::collections::VecDeque;

const LIMIT: usize = 2000;

#[derive(Debug, Default)]
pub struct Log {
    lines: VecDeque<String>,
    scroll: usize,
    viewport: usize,
}

impl Log {
    pub(crate) fn push(&mut self, text: &str) {
        for line in text.lines() {
            if self.lines.len() == LIMIT {
                self.lines.pop_front();
                self.scroll = self.scroll.saturating_sub(1);
            }
            self.lines.push_back(line.to_owned());
        }
    }

    pub fn lines(&self) -> impl Iterator<Item = &String> {
        self.lines.iter()
    }

    pub fn visible(&self, height: usize) -> impl Iterator<Item = &String> {
        let end = self.lines.len().saturating_sub(self.scroll);
        let start = end.saturating_sub(height);
        self.lines.iter().skip(start).take(end - start)
    }

    #[must_use]
    pub fn following(&self) -> bool {
        self.scroll == 0
    }

    pub fn set_viewport(&mut self, height: usize) {
        self.viewport = height;
    }

    #[must_use]
    pub fn viewport(&self) -> usize {
        self.viewport.max(1)
    }

    pub fn scroll(&mut self, delta: isize) {
        let ceiling = self.lines.len().saturating_sub(self.viewport);
        self.scroll = self.scroll.saturating_add_signed(-delta).min(ceiling);
    }
}
