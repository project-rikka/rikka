use std::collections::HashSet;
use std::ops::Range;

use files_core::Location;
use files_thumbs::Thumbnails;
use rikka_app::prelude::*;
use rikka_thumbnails::Size as ThumbSize;

use crate::services::Services;
use crate::tab::Tab;

const THUMBNAIL: ThumbSize = ThumbSize::Large;

impl Tab {
    pub(crate) fn show_range(&mut self, range: Range<usize>, services: &Services) {
        self.visible = range;
        let changed = self.refresh_thumbnails(services);
        self.repainted(changed);
    }

    pub(crate) fn refresh_thumbnails(&self, services: &Services) -> bool {
        let Some(thumbs) = services.thumbnails.as_ref() else {
            return false;
        };
        let visible = self.on_screen();

        services.showing(
            self.rows[visible.clone()]
                .iter()
                .map(|row| row.at.clone())
                .collect(),
        );
        let mut changed = false;
        for index in visible {
            if self.has_thumbnail(index) {
                continue;
            }
            if self.show_thumbnail(thumbs, index) {
                changed = true;
            } else if let Some(entry) = self.entry(index) {
                thumbs.request(entry, THUMBNAIL);
            }
        }
        changed
    }

    pub(crate) fn thumbnails_arrived(&self, services: &Services, arrived: Vec<Location>) {
        let Some(thumbs) = services.thumbnails.as_ref() else {
            skip_view();
            return;
        };
        let arrived: HashSet<Location> = arrived.into_iter().collect();
        let mut changed = false;
        for index in self.on_screen() {
            if arrived.contains(&self.rows[index].at) {
                changed |= self.show_thumbnail(thumbs, index);
            }
        }
        self.repainted(changed);
    }

    fn repainted(&self, changed: bool) {
        if changed {
            self.drawing.repaint();
        }
        skip_view();
    }

    fn has_thumbnail(&self, index: usize) -> bool {
        self.thumbnails
            .borrow()
            .get(index)
            .is_some_and(Option::is_some)
    }

    fn show_thumbnail(&self, thumbs: &Thumbnails, index: usize) -> bool {
        let Some(entry) = self
            .entry(index)
            .filter(|entry| thumbs.can_thumbnail(entry))
        else {
            return false;
        };
        let thumbnail = entry
            .modified
            .and_then(|modified| thumbs.ready(&entry.location, modified, THUMBNAIL));
        let mut thumbnails = self.thumbnails.borrow_mut();
        let Some(slot) = thumbnails.get_mut(index) else {
            return false;
        };
        let changed = *slot != thumbnail;
        *slot = thumbnail;
        changed
    }

    fn on_screen(&self) -> Range<usize> {
        let rows = self.rows.len();
        self.visible.start.min(rows)..self.visible.end.min(rows)
    }
}
