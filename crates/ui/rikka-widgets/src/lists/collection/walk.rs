use rikka_types::Key;

#[derive(Clone, Copy, Debug)]
pub(crate) struct KeyboardWalk {
    pub(crate) count: usize,
    pub(crate) columns: usize,
    pub(crate) page: usize,
}

impl KeyboardWalk {
    pub(crate) fn strip(count: usize) -> Self {
        Self {
            count,
            columns: count.max(1),
            page: 1,
        }
    }

    pub(crate) fn column(count: usize, page: usize) -> Self {
        Self {
            count,
            columns: 1,
            page: page.max(1),
        }
    }

    pub(crate) fn to(
        self,
        key: Key,
        from: Option<usize>,
        reachable: impl Fn(usize) -> bool,
    ) -> Option<Option<usize>> {
        if self.count == 0 {
            return None;
        }
        let across = self.columns.max(1) as isize;
        let banded = self.columns < self.count;
        let travel = match key {
            Key::ArrowUp if banded => -across,
            Key::ArrowDown if banded => across,
            Key::PageUp if banded => -((self.page * self.columns.max(1)) as isize),
            Key::PageDown if banded => (self.page * self.columns.max(1)) as isize,
            Key::ArrowLeft if across > 1 => -1,
            Key::ArrowRight if across > 1 => 1,
            Key::Home => return Some(self.slide(0, true, reachable)),
            Key::End => return Some(self.slide(self.count as isize - 1, false, reachable)),
            _ => return None,
        };
        let start = from.map_or(0, |index| index as isize + travel);
        Some(self.slide(start, travel >= 0, reachable))
    }

    fn slide(
        self,
        start: isize,
        forward: bool,
        reachable: impl Fn(usize) -> bool,
    ) -> Option<usize> {
        let mut index = start.clamp(0, self.count as isize - 1);
        while (0..self.count as isize).contains(&index) {
            if reachable(index as usize) {
                return Some(index as usize);
            }
            index += if forward { 1 } else { -1 };
        }
        None
    }
}
