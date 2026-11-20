use rikka_app::prelude::*;
use rikka_web::{ChosenWith, Dropdown, DropdownId, DropdownItem, DropdownOption, UpdateDropdown};

use crate::shell::Shell;
use browser_model::TabId;

const ANCHOR: &str = "rikka-browser-dropdown-anchor";
const MENU: &str = "rikka-browser-dropdown";
const MAX_WIDTH: f32 = 560.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum DropdownOwner {
    Tab(TabId),
    ActionPopup,
}

#[derive(Debug, Default)]
pub(crate) struct DropdownSlot {
    shown: Option<Shown>,
}

#[derive(Debug)]
struct Shown {
    dropdown: Dropdown,
    opened: u64,
}

impl DropdownSlot {
    pub(crate) fn is_open(&self) -> bool {
        self.shown.is_some()
    }

    fn open(&mut self, dropdown: Dropdown, opened: u64) {
        let replaced = self.shown.replace(Shown { dropdown, opened });
        drop(replaced);
    }

    fn update(&mut self, update: UpdateDropdown) {
        if let Some(Shown { dropdown, .. }) = &mut self.shown
            && dropdown.id == update.id
        {
            dropdown.items = update.items;
            dropdown.selected = update.selected;
        }
    }

    fn hide(&mut self, id: DropdownId) {
        if self
            .shown
            .as_ref()
            .is_some_and(|shown| shown.dropdown.id == id)
        {
            self.shown = None;
        }
    }

    fn take(&mut self) -> Option<Dropdown> {
        self.shown.take().map(|shown| shown.dropdown)
    }
}

impl Shell {
    pub(crate) fn dropdown_opened(&mut self, owner: DropdownOwner, dropdown: Dropdown) {
        let opened = self.browser().page_services().requests_mut().next_opening();
        self.with_dropdown(owner, |slot| slot.open(dropdown, opened));
    }

    pub(crate) fn dropdown_updated(&mut self, owner: DropdownOwner, update: UpdateDropdown) {
        self.with_dropdown(owner, |slot| slot.update(update));
    }

    pub(crate) fn dropdown_hidden(&mut self, owner: DropdownOwner, id: DropdownId) {
        self.with_dropdown(owner, |slot| slot.hide(id));
    }

    pub(crate) fn dismiss_dropdown(&mut self, owner: DropdownOwner) {
        if let Some(dropdown) = self.with_dropdown(owner, DropdownSlot::take) {
            dropdown.dismiss();
        }
    }

    fn choose_in_dropdown(&mut self, owner: DropdownOwner, index: u32, activation: Activation) {
        let Some(dropdown) = self.with_dropdown(owner, DropdownSlot::take) else {
            return;
        };
        let with = match activation {
            Activation::Pointer => ChosenWith::Pointer,
            Activation::Keyboard => ChosenWith::Keyboard,
        };
        dropdown.choose(index, with);
    }

    fn with_dropdown<R>(
        &mut self,
        owner: DropdownOwner,
        change: impl FnOnce(&mut DropdownSlot) -> R,
    ) -> R {
        match owner {
            DropdownOwner::Tab(tab) => {
                let page_services = self.browser().page_services();
                let mut requests = page_services.requests_mut();
                change(&mut requests.edit(tab).dropdown)
            }
            DropdownOwner::ActionPopup => change(&mut self.popup.dropdown),
        }
    }
}

struct Rows {
    items: Vec<MenuItem>,
    options: Vec<Option<u32>>,
    selected: Option<usize>,
}

impl Rows {
    fn of(items: &[DropdownItem], selected: Option<u32>) -> Rows {
        let mut rows = Rows {
            items: Vec::new(),
            options: Vec::new(),
            selected: None,
        };
        for item in items {
            match item {
                DropdownItem::Option(option) => rows.option(option, selected),
                DropdownItem::Separator => rows.push(menu_separator(), None),
                DropdownItem::Group(group) => {
                    rows.push(menu_header(group.label.clone()), None);
                    for option in &group.options {
                        rows.option(option, selected);
                    }
                }
            }
        }
        rows
    }

    fn option(&mut self, option: &DropdownOption, selected: Option<u32>) {
        let chosen = selected == Some(option.index);
        if chosen {
            self.selected = Some(self.items.len());
        }
        let item = menu_item(option.label.clone())
            .selected(chosen)
            .enabled(!option.disabled);
        self.push(item, Some(option.index));
    }

    fn push(&mut self, item: MenuItem, option: Option<u32>) {
        self.items.push(item);
        self.options.push(option);
    }
}

pub(crate) fn view(slot: &DropdownSlot, owner: DropdownOwner) -> Vec<Element> {
    let Some(Shown { dropdown, opened }) = &slot.shown else {
        return Vec::new();
    };
    let anchor = dropdown.anchor;
    let Rows {
        items,
        options,
        selected,
    } = Rows::of(&dropdown.items, dropdown.selected);
    let menu = rikka_widgets::menu()
        .items(items)
        .highlighted(selected)
        .fit_content()
        .min_width(anchor.width())
        .max_width(MAX_WIDTH.max(anchor.width()))
        .on_choose(
            move |shell: &mut Shell, row: usize, activation: Activation| {
                if let Some(index) = options.get(row).copied().flatten() {
                    shell.choose_in_dropdown(owner, index, activation);
                }
            },
        )
        .on_dismiss(move |shell: &mut Shell| shell.dismiss_dropdown(owner))
        .build()
        .keyed((MENU, *opened))
        .anchored(Anchor::new((ANCHOR, owner), AnchorEdge::BelowStart));
    vec![
        spacer()
            .width(anchor.width())
            .height(anchor.height())
            .position(anchor.x(), anchor.y())
            .build()
            .keyed((ANCHOR, owner)),
        menu,
    ]
}
