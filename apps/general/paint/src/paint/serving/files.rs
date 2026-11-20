use std::path::PathBuf;

use paint_assistant::{NewOrder, SaveOrder};
use paint_document::{Blank, FileFormat};
use rikka_app::prelude::*;

use super::{Answer, Paint, PendingReply, reply};

impl Paint {
    pub(super) fn save_for(&mut self, index: usize, order: SaveOrder, answer: Answer) {
        if let Err(refusal) = self.editable(index) {
            return reply(answer, Err(refusal));
        }
        let tab = &self.tabs[index];
        if tab.is_saving() {
            return reply(
                answer,
                Err(format!("\"{}\" is being saved already", self.handle(index))),
            );
        }
        let Some(path) = order.path.or_else(|| tab.path().map(PathBuf::from)) else {
            return reply(
                answer,
                Err(format!(
                    "\"{}\" has never been saved; save it with a `path` ending in .png, .jpg or .psd",
                    self.handle(index)
                )),
            );
        };
        let Some(format) = FileFormat::for_path(&path) else {
            return reply(
                answer,
                Err(format!(
                    "\"{}\" has no format Paint writes; save it with a `path` ending in .png, .jpg or .psd",
                    self.handle(index)
                )),
            );
        };
        let mut choices = self.choices(format);
        if let Some(quality) = order.quality {
            choices.quality = quality;
        }
        let id = tab.id();
        self.tabs[index].save_to(path, choices.saving());
        self.wait(id, PendingReply::Save, answer);
    }

    pub(super) fn open_for(&mut self, path: PathBuf, answer: Answer) {
        self.open_paths(vec![path]);
        let Some(tab) = self.active_tab() else {
            return reply(answer, Err("the file could not be opened".to_owned()));
        };
        let id = tab.id();
        let ready = tab.facts().is_some();
        self.wait(id, PendingReply::Open, answer);
        if ready {
            let described = self.describe(self.active);
            self.answer_all(id, |on| matches!(on, PendingReply::Open), described);
        }
    }

    pub(super) fn create_for(&mut self, made: NewOrder, answer: Answer) {
        let blank = Blank::new(made.width, made.height)
            .resolution(made.resolution)
            .backdrop(made.backdrop)
            .layer_name(tr!("paint.layers.numbered", number = 1).to_string());
        self.open_blank(blank);
        match self.active_tab() {
            Some(tab) => {
                let id = tab.id();
                self.wait(id, PendingReply::Open, answer);
            }
            None => reply(answer, Err("the image could not be made".to_owned())),
        }
    }
}
