use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use rikka_bluetooth::{Device, Request};

use crate::bluetooth::component::Bluetooth;
use crate::bluetooth::kind;
use crate::info_row::info_row;

const WIDE: f32 = 300.0;

pub(crate) fn view(device: &Device, label: &TextDocument) -> impl View {
    let trusted = device.id.clone();
    let forgotten = device.id.clone();
    let id = device.id.clone();

    column! {
        section(
            tr!("settings.bluetooth.detail.about"),
            column! {
                field(
                    tr!("settings.bluetooth.detail.name"),
                    row! {
                        text_field(label)
                            .on_change(|| {})
                            .on_submit(|bluetooth: &mut Bluetooth| bluetooth.relabel())
                            .on_blur(|bluetooth: &mut Bluetooth| bluetooth.relabel())
                            .width(WIDE);
                    },
                );
                info_row(tr!("settings.bluetooth.detail.kind"), kind::label(device.kind).to_string());
                info_row(tr!("settings.bluetooth.detail.address"), device.address.clone());
                device
                    .battery
                    .and_then(|battery| info_row(tr!("settings.bluetooth.detail.battery"), format!("{battery}%")));
                device
                    .signal
                    .and_then(|signal| info_row(tr!("settings.bluetooth.detail.signal"), format!("{signal} dBm")));
                info_row(
                    tr!("settings.bluetooth.detail.services"),
                    device.services.len().to_string(),
                );
                info_row(
                    tr!("settings.bluetooth.detail.pairing"),
                    match device.legacy_pairing {
                        true => tr!("settings.bluetooth.detail.legacy"),
                        false => tr!("settings.bluetooth.detail.secure"),
                    }
                    .to_string(),
                );
            }
            .gap(Space::SNUG)
            .fill_width(),
        );
        section(
            tr!("settings.bluetooth.detail.behaviour"),
            field(
                tr!("settings.bluetooth.detail.trusted"),
                switch(device.trusted).on_toggle(move |bluetooth: &mut Bluetooth, on: bool| {
                    bluetooth.send(Request::Trust(trusted.clone(), on));
                }),
            ),
        );
        row! {
            spacer();
            button(text(tr!("settings.bluetooth.detail.forget")))
                .danger()
                .on_press(move |bluetooth: &mut Bluetooth| bluetooth.forget(forgotten.clone()));
            if device.connected {
                button(text(tr!("settings.bluetooth.detail.disconnect")))
                    .secondary()
                    .on_press(move |bluetooth: &mut Bluetooth| {
                        bluetooth.send(Request::Disconnect(id.clone()));
                    });
            } else {
                button(text(tr!("settings.bluetooth.detail.connect")))
                    .primary()
                    .on_press(move |bluetooth: &mut Bluetooth| {
                        bluetooth.send(Request::Connect(id.clone()));
                    });
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
    }
    .gap(Space::ROOMY)
    .fill_width()
}
