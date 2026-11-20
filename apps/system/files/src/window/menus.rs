use rikka_app::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Menu {
    Context { at: Point, pastable: bool },
    Places { index: usize, at: Point },
    Tools,
    OpenWith,
    Sort,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Shown {
    pub(crate) menu: Menu,
    pub(crate) key: u64,
}

#[derive(Debug, Default, State)]
pub(crate) struct Menus {
    open: Tracked<Option<Shown>>,
    #[untracked]
    opened: u64,
}

impl Menus {
    pub(crate) fn show(&mut self, menu: Menu) {
        self.switch(Some(menu));
    }

    pub(crate) fn close(&mut self) {
        self.switch(None);
    }

    pub(crate) fn open(&self) -> Option<Shown> {
        *self.open
    }

    fn switch(&mut self, to: Option<Menu>) {
        if self.open.map(|shown| shown.menu) == to {
            return;
        }
        let shown = to.map(|menu| {
            self.opened += 1;
            Shown {
                menu,
                key: self.opened,
            }
        });
        self.open.set(shown);
    }
}

#[derive(Default)]
pub(crate) struct Rows {
    items: Vec<MenuItem>,
    ruled: bool,
}

impl Rows {
    pub(crate) fn command<M>(
        &mut self,
        label: Label,
        symbol: MaterialSymbol,
        press: impl IntoHandler<(), (), M>,
    ) {
        self.push(menu_item(label).icon(symbol), press);
    }

    pub(crate) fn keyed<M>(
        &mut self,
        label: Label,
        symbol: MaterialSymbol,
        shortcut: String,
        press: impl IntoHandler<(), (), M>,
    ) {
        self.push(menu_item(label).icon(symbol).shortcut(shortcut), press);
    }

    pub(crate) fn toggle<M>(
        &mut self,
        label: Label,
        symbol: MaterialSymbol,
        on: bool,
        press: impl IntoHandler<(), (), M>,
    ) {
        self.push(menu_item(label).icon(symbol).selected(on), press);
    }

    pub(crate) fn choice<M>(
        &mut self,
        label: impl Into<Label>,
        chosen: bool,
        press: impl IntoHandler<(), (), M>,
    ) {
        self.push(menu_item(label).selected(chosen), press);
    }

    pub(crate) fn item<M>(&mut self, label: impl Into<Label>, press: impl IntoHandler<(), (), M>) {
        self.push(menu_item(label), press);
    }

    pub(crate) fn branch(&mut self, label: Label, symbol: MaterialSymbol, children: Rows) {
        let children = children.finish();
        if children.is_empty() {
            return;
        }
        self.items
            .push(menu_item(label).icon(symbol).items(children));
        self.ruled = false;
    }

    pub(crate) fn rule(&mut self) {
        if self.items.is_empty() || self.ruled {
            return;
        }
        self.items.push(menu_separator());
        self.ruled = true;
    }

    pub(crate) fn finish(mut self) -> Vec<MenuItem> {
        if self.ruled {
            self.items.pop();
        }
        self.items
    }

    fn push<M>(&mut self, item: MenuItem, press: impl IntoHandler<(), (), M>) {
        let press = press.into_handler();
        self.items
            .push(item.on_press(Handler::relaying(move |reach, ()| {
                reach.get::<Menus>()?.close();
                press.call(reach, ())
            })));
        self.ruled = false;
    }
}
