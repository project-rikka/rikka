use rikka_bluetooth::{Device, State as BluetoothState};
use rikka_ui::{Element, Label, View, tr};
use rikka_widgets::{ControlSize, Switch, switch};
use shell_feeds::symbols;

use crate::centre::detail::Detail;
use crate::centre::state::Centre;
use crate::centre::view::quick::{Quick, quick};
use crate::centre::view::tile;

pub(crate) fn tile(bluetooth: &BluetoothState) -> Quick {
    let here = bluetooth.radio().is_some();
    let on = bluetooth.is_on();
    let connected = bluetooth
        .known()
        .into_iter()
        .find(|device| device.connected);

    quick(symbols::bluetooth(on, connected), Detail::Bluetooth.title())
        .taskbar_visibility(taskbar_visibility(on, connected))
        .on(on)
        .here(here)
        .toggle(move |centre: &mut Centre| centre.daemons().bluetooth().use_radio(!on))
        .opens(Detail::Bluetooth)
}

pub(crate) fn radio(bluetooth: &BluetoothState) -> Switch {
    switch(bluetooth.is_on())
        .control_size(ControlSize::Medium)
        .enabled(bluetooth.radio().is_some())
        .on_toggle(|centre: &mut Centre, on| centre.daemons().bluetooth().use_radio(on))
}

pub(crate) fn page(bluetooth: &BluetoothState) -> Vec<Element> {
    let mut rows = Vec::new();
    if bluetooth.is_on() {
        let known = bluetooth.known();
        match known.is_empty() {
            true => rows.push(tile::caption(tr!("shell.centre.nothing-paired")).build()),
            false => rows.extend(known.into_iter().map(line)),
        }
    }
    if let Some(refused) = &bluetooth.error {
        rows.push(tile::refusal(&refused.reason).build());
    }
    rows
}

fn taskbar_visibility(on: bool, connected: Option<&Device>) -> Label {
    match (on, connected) {
        (false, _) => tr!("shell.centre.off"),
        (true, Some(device)) => Label::from(device.title.as_str()),
        (true, None) => tr!("shell.centre.not-connected"),
    }
}

fn line(device: &Device) -> Element {
    let detail = device
        .battery
        .map(|charge| Label::from(format!("{charge}%")));
    let (id, connected) = (device.id.clone(), device.connected);
    tile::choice(
        symbols::device(device.kind),
        Label::from(device.title.as_str()),
        detail,
        connected,
        move |centre: &mut Centre| centre.connect(&id, !connected),
    )
}
