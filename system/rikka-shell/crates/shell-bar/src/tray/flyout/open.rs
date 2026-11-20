use rikka_tray::{Entry, ItemId, Menu};

#[derive(Clone, Debug)]
pub(crate) struct Open {
    pub(crate) item: ItemId,
    pub(crate) row: usize,
    unfolded: Vec<(usize, f32)>,
}

impl Open {
    #[must_use]
    pub(crate) fn new(item: ItemId, row: usize) -> Open {
        Open {
            item,
            row,
            unfolded: Vec::new(),
        }
    }

    pub(super) fn fold_under(&mut self, level: usize) {
        self.unfolded.truncate(level);
    }

    pub(crate) fn unfold(&mut self, level: usize, row: usize, at: f32) {
        self.fold_under(level);
        self.unfolded.push((row, at));
    }

    #[must_use]
    pub(super) fn opened_at(&self, level: usize) -> Option<f32> {
        level
            .checked_sub(1)
            .and_then(|above| self.unfolded.get(above))
            .map(|(_, at)| *at)
    }

    #[must_use]
    pub(crate) fn levels<'a>(&self, menu: &'a Menu) -> Vec<&'a [Entry]> {
        let mut levels = vec![menu.entries.as_slice()];
        for (row, _) in &self.unfolded {
            let Some(Entry::Row(row)) = levels.last().and_then(|entries| entries.get(*row)) else {
                break;
            };
            if row.entries.is_empty() {
                break;
            }
            levels.push(row.entries.as_slice());
        }
        levels
    }
}
