use std::rc::Rc;

use rikka_ui::MaterialSymbol;
use rikka_widgets::{MenuItem, menu_header, menu_item, menu_separator};

use crate::ffi;
use crate::handlers::{Callback, shared_action};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    Item,
    Submenu,
    Header,
    Separator,
}

pub(crate) struct MenuEntry {
    pub(crate) role: Role,
    pub(crate) label: String,
    pub(crate) shortcut: Option<String>,
    pub(crate) icon: Option<MaterialSymbol>,
    pub(crate) checked: bool,
    pub(crate) enabled: bool,
    pub(crate) press: Option<Rc<Callback<ffi::ActionFn>>>,
    pub(crate) children: Vec<MenuEntry>,
}

impl MenuEntry {
    pub(crate) fn new(role: Role, label: &str) -> Self {
        Self {
            role,
            label: label.to_owned(),
            shortcut: None,
            icon: None,
            checked: false,
            enabled: true,
            press: None,
            children: Vec::new(),
        }
    }

    pub(crate) fn to_item(&self) -> MenuItem {
        let mut item = match self.role {
            Role::Item => menu_item(self.label.as_str()),
            Role::Submenu => menu_item(self.label.as_str())
                .submenu(true)
                .items(self.children.iter().map(MenuEntry::to_item)),
            Role::Header => menu_header(self.label.as_str()),
            Role::Separator => menu_separator(),
        };
        if let Some(shortcut) = &self.shortcut {
            item = item.shortcut(shortcut.as_str());
        }
        if let Some(icon) = self.icon {
            item = item.icon(icon);
        }
        if let Some(press) = &self.press {
            item = item.on_press(shared_action(Rc::clone(press)));
        }
        item.selected(self.checked).enabled(self.enabled)
    }
}

pub(crate) fn items(entries: &[MenuEntry]) -> Vec<MenuItem> {
    entries.iter().map(MenuEntry::to_item).collect()
}
