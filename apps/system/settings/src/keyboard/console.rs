use rikka_app::prelude::*;
use rikka_input::Layouts;
use rikka_nix::Module;
use rikka_widgets::column;

use crate::keyboard::options;
use crate::pending::Pending;
use crate::picker::value_picker;

const LAYOUT: &str = "services.xserver.xkb.layout";
const VARIANT: &str = "services.xserver.xkb.variant";
const KEY_MAP: &str = "console.keyMap";

pub(crate) fn card(layouts: &Layouts, keymaps: &[String], pending: &Pending) -> impl View {
    let layout = pending.get::<String>(Module::System, LAYOUT);
    section(
        tr!("settings.keyboard.console"),
        column! {
            field(
                tr!("settings.keyboard.console-layout"),
                value_picker(
                    tr!("settings.keyboard.unset"),
                    options::layouts(layouts),
                    layout.as_deref(),
                    |pending: &mut Pending, layout: Option<String>| {
                        choose_layout(pending, layout.as_deref());
                    },
                ),
            );
            field(
                tr!("settings.keyboard.console-variant"),
                value_picker(
                    tr!("settings.keyboard.plain-layout"),
                    options::variants(layout.as_deref().and_then(|name| layouts.layout(name))),
                    pending.get::<String>(Module::System, VARIANT).as_deref(),
                    |pending: &mut Pending, variant: Option<String>| {
                        choose_variant(pending, variant.as_deref());
                    },
                ),
            );
            field(
                tr!("settings.keyboard.console-keymap"),
                value_picker(
                    tr!("settings.keyboard.unset"),
                    options::named(keymaps),
                    pending.get::<String>(Module::System, KEY_MAP).as_deref(),
                    |pending: &mut Pending, keymap: Option<String>| {
                        choose_keymap(pending, keymap.as_deref());
                    },
                ),
            );
        }
        .gap(Space::SNUG),
    )
}

fn choose_layout(pending: &mut Pending, layout: Option<&str>) {
    pending.write(Module::System, LAYOUT, layout);
    pending.remove(Module::System, VARIANT);
}

fn choose_variant(pending: &mut Pending, variant: Option<&str>) {
    pending.write(Module::System, VARIANT, variant);
}

fn choose_keymap(pending: &mut Pending, keymap: Option<&str>) {
    pending.write(Module::System, KEY_MAP, keymap);
}
