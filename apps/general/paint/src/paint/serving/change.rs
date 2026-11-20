use paint_assistant::Change;

use super::compose::{Defaults, compose};
use super::{Answer, Paint, PendingReply, reply};
use crate::tab::Composition;

impl Paint {
    pub(super) fn change(&mut self, index: usize, name: String, change: Change, answer: Answer) {
        let active_layer = match self.editable(index) {
            Ok(tab) => tab.active_layer().map(|layer| layer.id),
            Err(refusal) => return reply(answer, Err(refusal)),
        };
        let defaults = Defaults {
            colors: self.colors,
            options: self.options.clone(),
        };
        let lists = matches!(change, Change::Layers(_));
        let reshapes = matches!(change, Change::Transform(_));
        let composition = Composition::new(name.clone(), move |document| {
            compose(document, change, active_layer, &defaults)
        });
        let tab = &mut self.tabs[index];
        let id = tab.id();
        match tab.compose(composition, reshapes) {
            Some(generation) => self.wait(
                id,
                PendingReply::Change {
                    generation,
                    name,
                    lists,
                },
                answer,
            ),
            None => reply(
                answer,
                Err("the document has no worker to change it".to_owned()),
            ),
        }
    }
}
