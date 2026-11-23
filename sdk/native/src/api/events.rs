use std::rc::Rc;

use rikka_ui::Command;
use rikka_widgets::command_label;

use crate::ffi;
use crate::handlers::{Callback, action, flag, index, number};
use crate::input;
use crate::view::View;
use crate::view::node::{Kind, Node};

pub(crate) fn on_press(view: &mut View, callback: Callback<ffi::ActionFn>) {
    let node = view.current();
    match &mut node.kind {
        Kind::Button(button) | Kind::ButtonFrame { button, .. } => {
            let handler = action(callback);
            button.edit(move |button| button.on_press(handler));
        }
        Kind::Box { container, .. } => {
            let handler = action(callback);
            container.edit(move |container| container.on_press(handler));
        }
        Kind::Radio(radio) => {
            let handler = action(callback);
            radio.edit(move |radio| radio.on_select(handler));
        }
        Kind::MenuEntry(entry) => entry.press = Some(Rc::new(callback)),
        _ => node.refuse_reaction("presses"),
    }
}

pub(crate) fn on_toggle(view: &mut View, callback: Callback<ffi::BoolFn>) {
    let handler = flag(callback);
    let node = view.current();
    match &mut node.kind {
        Kind::Checkbox(checkbox) => checkbox.edit(move |checkbox| checkbox.on_toggle(handler)),
        Kind::Toggle(toggle) => toggle.edit(move |toggle| toggle.on_toggle(handler)),
        Kind::ListItem(item) => item.edit(move |item| item.on_check(handler)),
        _ => node.refuse_reaction("toggles"),
    }
}

pub(crate) fn on_value(view: &mut View, callback: Callback<ffi::DoubleFn>) {
    let handler = number(callback);
    let node = view.current();
    match &mut node.kind {
        Kind::Slider(slider) => slider.edit(move |slider| slider.on_change(handler)),
        Kind::NumberField(number) => number.edit(move |number| number.on_commit(handler)),
        _ => node.refuse_reaction("value changes"),
    }
}

pub(crate) fn on_value_commit(view: &mut View, callback: Callback<ffi::DoubleFn>) {
    let handler = number(callback);
    let node = view.current();
    match &mut node.kind {
        Kind::Slider(slider) => slider.edit(move |slider| slider.on_commit(handler)),
        _ => node.refuse_reaction("value commits"),
    }
}

pub(crate) fn on_select(view: &mut View, callback: Callback<ffi::IndexFn>) {
    let handler = index(callback);
    let node = view.current();
    match &mut node.kind {
        Kind::Segmented(segmented) => segmented.edit(move |segmented| segmented.on_select(handler)),
        Kind::Combo(combo) => combo.edit(move |combo| combo.on_select(handler)),
        Kind::Tabs(tabs) => tabs.edit(move |tabs| tabs.on_select(handler)),
        Kind::NavList(nav) => nav.edit(move |nav| nav.on_select(handler)),
        Kind::List(list) => list.edit(move |list| list.on_select(handler)),
        Kind::LazyList { list, .. } => list.edit(move |list| list.on_select(handler)),
        Kind::Collection(collection) => {
            collection.edit(move |collection| collection.on_select(handler));
        }
        _ => node.refuse_reaction("selection"),
    }
}

pub(crate) fn on_activate(view: &mut View, callback: Callback<ffi::IndexFn>) {
    let handler = index(callback);
    let node = view.current();
    match &mut node.kind {
        Kind::List(list) => list.edit(move |list| list.on_activate(handler)),
        Kind::LazyList { list, .. } => list.edit(move |list| list.on_activate(handler)),
        Kind::Collection(collection) => {
            collection.edit(move |collection| collection.on_activate(handler));
        }
        _ => node.refuse_reaction("activation"),
    }
}

pub(crate) fn on_change(view: &mut View, callback: Callback<ffi::ActionFn>) {
    let handler = action(callback);
    let node = view.current();
    match &mut node.kind {
        Kind::TextField(field) => field.edit(move |field| field.on_change(handler)),
        Kind::TextEditor(editor) => editor.edit(move |editor| editor.on_change(handler)),
        _ => node.refuse_reaction("edits"),
    }
}

pub(crate) fn on_submit(view: &mut View, callback: Callback<ffi::ActionFn>) {
    let handler = action(callback);
    let node = view.current();
    match &mut node.kind {
        Kind::TextField(field) => field.edit(move |field| field.on_submit(handler)),
        Kind::NumberField(number) => number.edit(move |number| number.on_submit(handler)),
        _ => node.refuse_reaction("submission"),
    }
}

pub(crate) fn on_blur(view: &mut View, callback: Callback<ffi::ActionFn>) {
    let handler = action(callback);
    let node = view.current();
    match &mut node.kind {
        Kind::TextField(field) => field.edit(move |field| field.on_blur(handler)),
        Kind::TextEditor(editor) => editor.edit(move |editor| editor.on_blur(handler)),
        _ => node.refuse_reaction("focus leaving"),
    }
}

pub(crate) fn on_dismiss(view: &mut View, callback: Callback<ffi::ActionFn>) {
    let handler = action(callback);
    let node = view.current();
    match &mut node.kind {
        Kind::Sheet { dismiss, .. }
        | Kind::Popover { dismiss, .. }
        | Kind::PopupMenu { dismiss }
        | Kind::Scrim { dismiss } => *dismiss = Some(handler),
        Kind::Banner(banner) => {
            banner.edit(move |banner| banner.on_dismiss(command_label(Command::Close), handler));
        }
        _ => node.refuse_reaction("dismissal"),
    }
}

pub(crate) fn on_close(view: &mut View, callback: Callback<ffi::IndexFn>) {
    let handler = index(callback);
    let node = view.current();
    match &mut node.kind {
        Kind::Tabs(tabs) => tabs.edit(move |tabs| tabs.on_close(handler)),
        _ => node.refuse_reaction("tab closing"),
    }
}

pub(crate) fn on_new(view: &mut View, callback: Callback<ffi::ActionFn>) {
    let handler = action(callback);
    let node = view.current();
    match &mut node.kind {
        Kind::Tabs(tabs) => tabs.edit(move |tabs| tabs.on_new(handler)),
        _ => node.refuse_reaction("new tabs"),
    }
}

pub(crate) fn on_hover(view: &mut View, callback: Callback<ffi::BoolFn>) {
    let handler = flag(callback);
    let node = view.current();
    match &mut node.kind {
        Kind::Box { container, .. } => container.edit(move |container| container.on_hover(handler)),
        _ => node.refuse_reaction("hovering"),
    }
}

pub(crate) fn on_resize(view: &mut View, callback: Callback<ffi::DoubleFn>) {
    let handler = number(callback);
    let node = view.current();
    match &mut node.kind {
        Kind::Split { resize, .. } => *resize = Some(handler),
        _ => node.refuse_reaction("resizing"),
    }
}

fn droppable(view: &mut View) -> &mut Node {
    let node = view.current();
    if !node.kind.is_element() {
        node.refuse_reaction("drops");
    }
    node
}

pub(crate) fn on_drop_files(view: &mut View, callback: Callback<ffi::PathsFn>) {
    droppable(view).common.drops.files = Some(Rc::new(callback));
}

pub(crate) fn on_drop_text(view: &mut View, callback: Callback<ffi::StringFn>) {
    droppable(view).common.drops.text = Some(Rc::new(callback));
}

fn bindable(node: &Node) {
    if !node.kind.is_element() {
        node.refuse_reaction("key bindings");
    }
}

pub(crate) fn on_shortcut(
    view: &mut View,
    key: ffi::KeyCode,
    modifiers: ffi::Modifiers,
    callback: Callback<ffi::ActionFn>,
) {
    let key = input::key(key);
    let modifiers = input::modifiers(modifiers);
    let node = view.current();
    bindable(node);
    node.common
        .shortcuts
        .push((key, modifiers, action(callback)));
}

pub(crate) fn on_command(
    view: &mut View,
    command: ffi::Command,
    callback: Callback<ffi::ActionFn>,
) {
    let command = input::command(command);
    let node = view.current();
    bindable(node);
    node.common.commands.push((command, action(callback)));
}
