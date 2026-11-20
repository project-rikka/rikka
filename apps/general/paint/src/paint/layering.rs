use paint_document::{LayerId, Placement};
use rikka_app::prelude::*;

use super::Paint;
use crate::tab::{Layering, Tab};

impl Paint {
    pub(crate) fn can_arrange(&self, order: Layering) -> bool {
        self.unsaved_prompt.is_none()
            && self
                .active_tab()
                .is_some_and(|tab| tab.layering(order).is_some())
    }

    pub(crate) fn arrange(&mut self, order: Layering) {
        self.edit(|tab| tab.arrange(order));
    }

    pub(crate) fn can_restyle(&self) -> bool {
        self.unsaved_prompt.is_none() && self.active_tab().is_some_and(Tab::restyles)
    }

    pub(crate) fn aim_layer(&mut self, aim: Option<Placement>) {
        if !self.active_tab_mut().is_some_and(|tab| tab.aim_at(aim)) {
            skip_view();
        }
    }

    pub(crate) fn drop_layer(&mut self, layer: LayerId, to: Placement) {
        self.edit(|tab| tab.drop_layer(layer, to));
    }

    pub(crate) fn open_layer_menu(&mut self, layer: LayerId, at: Point) {
        self.edit(|tab| tab.open_layer_menu(layer, at));
    }

    pub(crate) fn close_layer_menu(&mut self) {
        if let Some(tab) = self.active_tab_mut() {
            tab.close_layer_menu();
        }
    }
}
