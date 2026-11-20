use rikka_widgets as widgets;

use crate::values::symbol;
use crate::view::View;
use crate::view::menu::{MenuEntry, Role};
use crate::view::node::{Built, Kind, Segment};

pub(crate) fn segment(view: &mut View, label: &str) {
    view.leaf(Kind::Segment(Segment {
        label: label.to_owned(),
        icon: None,
    }));
}

pub(crate) fn icon_segment(view: &mut View, icon: &str, label: &str) {
    view.leaf(Kind::Segment(Segment {
        label: label.to_owned(),
        icon: Some(symbol(icon)),
    }));
}

pub(crate) fn option(view: &mut View, label: &str) {
    view.leaf(Kind::Option(label.to_owned()));
}

pub(crate) fn tab(view: &mut View, label: &str) {
    view.leaf(Kind::Tab(Built::ready(widgets::tab(label.to_owned()))));
}

pub(crate) fn nav_item(view: &mut View, label: &str) {
    view.leaf(Kind::NavItem(Built::ready(widgets::nav_item(
        label.to_owned(),
    ))));
}

pub(crate) fn nav_header(view: &mut View, label: &str) {
    view.leaf(Kind::NavItem(Built::ready(widgets::nav_header(
        label.to_owned(),
    ))));
}

pub(crate) fn list_item(view: &mut View, label: &str) {
    view.leaf(Kind::ListItem(Built::ready(widgets::list_item(
        label.to_owned(),
    ))));
}

pub(crate) fn list_header(view: &mut View, label: &str) {
    view.leaf(Kind::ListItem(Built::ready(widgets::list_header(
        label.to_owned(),
    ))));
}

pub(crate) fn menu_item(view: &mut View, label: &str) {
    view.leaf(Kind::MenuEntry(MenuEntry::new(Role::Item, label)));
}

pub(crate) fn menu_header(view: &mut View, label: &str) {
    view.leaf(Kind::MenuEntry(MenuEntry::new(Role::Header, label)));
}

pub(crate) fn menu_separator(view: &mut View) {
    view.leaf(Kind::MenuEntry(MenuEntry::new(Role::Separator, "")));
}
