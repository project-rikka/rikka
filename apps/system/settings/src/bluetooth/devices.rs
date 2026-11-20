use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use rikka_bluetooth::{Action, AdapterId, Device, DeviceId, Request, Snapshot};

use crate::bluetooth::component::Bluetooth;
use crate::bluetooth::kind;

pub(crate) fn known(
    snapshot: &Snapshot,
    adapter: &AdapterId,
    busy: Option<&DeviceId>,
) -> Option<impl View> {
    let devices = snapshot.known(adapter);
    if devices.is_empty() {
        return None;
    }
    Some(card(
        column! {
            text(tr!("settings.bluetooth.known")).body().semibold();
            for device in devices {
                known_row(device, busy == Some(&device.id));
            }
        }
        .gap(Space::SNUG)
        .fill_width(),
    ))
}

pub(crate) fn nearby(
    snapshot: &Snapshot,
    adapter: &AdapterId,
    discovering: bool,
    busy: Option<&DeviceId>,
    pairing: Option<Action>,
) -> impl View {
    card(
        column! {
            row! {
                text(tr!("settings.bluetooth.nearby")).body().semibold();
                spacer();
                if discovering {
                    spinner().extent(Length::px(16.0));
                }
                button(icon(if discovering {
                    MaterialSymbol::PAUSE
                } else {
                    MaterialSymbol::REFRESH
                }))
                .ghost()
                .small()
                .icon_only()
                .on_press(move |bluetooth: &mut Bluetooth| bluetooth.scan(!discovering));
            }
            .gap(Space::SNUG)
            .align_items(Align::Center);
            for device in snapshot.nearby(adapter) {
                nearby_row(device, busy == Some(&device.id) && pairing == Some(Action::Pair));
            }
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
}

fn nearby_row(device: &Device, pairing: bool) -> impl View {
    let id = device.id.clone();
    row! {
        icon(kind::symbol(device.kind))
            .extent(Length::px(20.0))
            .color(palette().text_muted);
        column! {
            text(device.title.as_str()).label();
            text(device.address.as_str()).caption().muted();
        }
        .gap(Space::HAIR);
        spacer();
        button(text(tr!("settings.bluetooth.pair")))
            .secondary()
            .small()
            .loading(pairing)
            .on_press(move |bluetooth: &mut Bluetooth| bluetooth.send(Request::Pair(id.clone())));
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .fill_width()
}

fn known_row(device: &Device, waiting: bool) -> impl View {
    let palette = palette();
    let inspected = device.id.clone();
    let id = device.id.clone();
    row! {
        icon(kind::symbol(device.kind))
            .extent(Length::px(20.0))
            .color(if device.connected {
                palette.accent
            } else {
                palette.text_muted
            });
        column! {
            text(device.title.as_str()).label();
            row! {
                text(if device.connected {
                    tr!("settings.bluetooth.connected")
                } else {
                    tr!("settings.bluetooth.paired")
                })
                .caption()
                .muted();
                if let Some(battery) = device.battery {
                    row! {
                        icon(kind::battery(battery))
                            .extent(Length::px(14.0))
                            .color(if battery < 15 {
                                palette.danger
                            } else {
                                palette.text_muted
                            });
                        text(format!("{battery}%")).caption().muted();
                    }
                    .gap(Space::HAIR)
                    .align_items(Align::Center);
                }
            }
            .gap(Space::SNUG)
            .align_items(Align::Center);
        }
        .gap(Space::HAIR);
        spacer();
        button(icon(MaterialSymbol::INFO))
            .ghost()
            .small()
            .icon_only()
            .on_press(move |bluetooth: &mut Bluetooth| bluetooth.inspect(inspected.clone()));
        if waiting {
            spinner().extent(Length::px(18.0));
        } else {
            switch(device.connected).on_toggle(move |bluetooth: &mut Bluetooth, on: bool| {
                bluetooth.send(match on {
                    true => Request::Connect(id.clone()),
                    false => Request::Disconnect(id.clone()),
                });
            });
        }
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .fill_width()
}
