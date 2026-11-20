use std::sync::atomic::{AtomicU32, Ordering};

use paint_document::Blank;
use rikka_app::prelude::*;

use super::Paint;
use crate::colors::Slot;
use crate::new_image::NewImage;
use crate::tab::Tab;

static NEXT_UNTITLED: AtomicU32 = AtomicU32::new(1);

impl Paint {
    pub(crate) fn creating(&self) -> Option<&NewImage> {
        self.creating.as_ref()
    }

    pub(crate) fn begin_new_image(&mut self) {
        if self.unsaved_prompt.is_none() {
            self.creating = Some(NewImage::default());
        }
    }

    pub(crate) fn tune_new_image(&mut self, change: impl FnOnce(&mut NewImage)) {
        if let Some(creating) = &mut self.creating {
            change(creating);
        }
    }

    pub(crate) fn cancel_new_image(&mut self) {
        self.creating = None;
    }

    pub(crate) fn create_image(&mut self) {
        let Some(creating) = self.creating.take() else {
            return;
        };
        let blank = creating.blank(
            self.colors.get(Slot::Primary),
            tr!("paint.layers.numbered", number = 1).to_string(),
        );
        self.open_blank(blank);
    }

    pub(super) fn open_blank(&mut self, blank: Blank) {
        let number = NEXT_UNTITLED.fetch_add(1, Ordering::Relaxed);
        let untitled = tr!("paint.new_image.untitled", number = number).to_string();
        self.tabs
            .push(Tab::create(blank, untitled, self.sender.clone()));
        self.active = self.tabs.len() - 1;
        self.notice = None;
        self.retitle();
    }
}
