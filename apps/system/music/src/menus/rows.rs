use rikka_app::prelude::*;
use rikka_ui::IntoHandler;

use crate::menus::Branch;

#[derive(Default)]
pub(super) struct Rows {
    items: Vec<MenuItem>,
    branches: Vec<Option<Branch>>,
    ruled: bool,
}

impl Rows {
    pub(super) fn item<M>(
        &mut self,
        label: Label,
        symbol: MaterialSymbol,
        press: impl IntoHandler<(), (), M>,
    ) {
        self.item_if(true, label, symbol, press);
    }

    pub(super) fn item_if<M>(
        &mut self,
        enabled: bool,
        label: Label,
        symbol: MaterialSymbol,
        press: impl IntoHandler<(), (), M>,
    ) {
        let item = menu_item(label)
            .icon(symbol)
            .enabled(enabled)
            .on_press(press);
        self.push(item, None);
    }

    pub(super) fn quiet(&mut self, label: Label) {
        self.push(menu_item(label).enabled(false), None);
    }

    pub(super) fn branch(&mut self, label: Label, symbol: MaterialSymbol, branch: Branch) {
        self.push(menu_item(label).icon(symbol).submenu(true), Some(branch));
    }

    pub(super) fn rule(&mut self) {
        if self.items.is_empty() || self.ruled {
            return;
        }
        self.push(menu_separator(), None);
        self.ruled = true;
    }

    pub(super) fn finish(mut self) -> (Vec<MenuItem>, Vec<Option<Branch>>) {
        if self.ruled {
            self.items.pop();
            self.branches.pop();
        }
        (self.items, self.branches)
    }

    fn push(&mut self, item: MenuItem, branch: Option<Branch>) {
        self.items.push(item);
        self.branches.push(branch);
        self.ruled = false;
    }
}
