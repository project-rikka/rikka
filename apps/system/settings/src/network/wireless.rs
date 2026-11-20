use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use rikka_network::{Action, Bars, Device, Request, Snapshot};

use crate::network::component::Network;
use crate::network::{format, strength};
use crate::notice::notice;
use crate::picker::picker;

pub(crate) fn view(
    snapshot: &Snapshot,
    radios: &[&Device],
    selected: Option<&Device>,
    busy: Option<Action>,
    looking: bool,
) -> impl View {
    column! {
        radio_card(snapshot, radios, selected);
        if let Some(device) = selected {
            if snapshot.wireless.enabled {
                connected(snapshot, device);
                networks(snapshot, device, busy, looking);
                actions(device);
            }
        } else {
            notice(
                MaterialSymbol::WIFI_OFF,
                palette().text_muted,
                tr!("settings.network.wireless.none"),
            );
        }
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn radio_card(snapshot: &Snapshot, radios: &[&Device], selected: Option<&Device>) -> impl View {
    section(
        tr!("settings.network.wireless.title"),
        column! {
            field(
                tr!("settings.network.wireless.enabled"),
                switch(snapshot.wireless.enabled)
                    .enabled(snapshot.wireless.hardware)
                    .on_toggle(|network: &mut Network, on: bool| {
                        network.send(Request::Radio(on));
                    }),
            );
            if !snapshot.wireless.hardware {
                text(tr!("settings.network.wireless.hardware-off"))
                    .caption()
                    .color(TextColor::Warning);
            }
            if radios.len() > 1 {
                field(
                    tr!("settings.network.wireless.radio"),
                    picker(
                        selected.and_then(|selected| radios.iter().position(|radio| radio.id == selected.id)),
                        radios.iter().map(|radio| Label::from(radio.title())),
                        |network: &mut Network, index: usize| network.choose_radio(index),
                    ),
                );
            }
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
}

fn connected(snapshot: &Snapshot, device: &Device) -> Option<impl View> {
    let active = snapshot.active_on(&device.id)?;
    let point = device
        .wireless
        .as_ref()
        .and_then(|wireless| wireless.access_point.as_ref())
        .and_then(|id| snapshot.access_points.iter().find(|point| &point.id == id));
    let inspected = active.uuid.clone();
    let stopped = active.id.clone();

    Some(section(
        tr!("settings.network.wireless.connected"),
        row! {
            icon(match point {
                Some(point) => strength::symbol(point.bars(), point.security),
                None => MaterialSymbol::WIFI,
            })
            .extent(Length::px(22.0))
            .color(palette().accent);
            column! {
                text(active.title.as_str()).body().semibold();
                text(format::state(device.state, device.failure)).caption().muted();
                format::connectivity(snapshot.connectivity)
                    .map(|connectivity| text(connectivity).caption().color(TextColor::Warning));
            }
            .gap(Space::HAIR);
            spacer();
            button(text(tr!("settings.network.wireless.details")))
                .secondary()
                .small()
                .on_press(move |network: &mut Network| network.inspect(&inspected));
            button(text(tr!("settings.network.wireless.disconnect")))
                .secondary()
                .small()
                .on_press(move |network: &mut Network| network.stop(stopped.clone()));
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width(),
    ))
}

fn networks(
    snapshot: &Snapshot,
    device: &Device,
    busy: Option<Action>,
    looking: bool,
) -> impl View {
    let networks = snapshot.networks(&device.id);
    card(
        column! {
            row! {
                text(tr!("settings.network.wireless.available")).body().semibold();
                spacer();
                if looking || busy == Some(Action::Scan) {
                    spinner().extent(Length::px(16.0));
                }
                button(icon(MaterialSymbol::REFRESH))
                    .ghost()
                    .small()
                    .icon_only()
                    .on_press(|network: &mut Network| network.scan());
            }
            .gap(Space::SNUG)
            .align_items(Align::Center);
            column! {
                for (index, network) in networks.iter().enumerate() {
                    if !network.in_use {
                        let saved = network.saved.clone();
                        let inspected = network.saved.clone();
                        button(
                            row! {
                                icon(strength::symbol(Bars::from_strength(network.strength), network.security))
                                    .extent(Length::px(20.0))
                                    .color(palette().text_muted);
                                column! {
                                    text(network.ssid.name().into_owned()).label();
                                    if network.saved.is_some() {
                                        text(tr!("settings.network.wireless.saved")).caption().muted();
                                    }
                                }
                                .gap(Space::HAIR);
                                spacer();
                                text(format!("{}%", network.strength)).caption().muted();
                                row! {
                                    if let Some(uuid) = inspected {
                                        button(icon(MaterialSymbol::INFO))
                                            .ghost()
                                            .small()
                                            .icon_only()
                                            .on_press(move |network: &mut Network| network.inspect(&uuid));
                                    }
                                }
                                .gap(Space::TIGHT)
                                .align_items(Align::Center);
                            }
                            .gap(Space::NORMAL)
                            .align_items(Align::Center)
                            .fill_width(),
                        )
                        .ghost()
                        .on_press(move |network: &mut Network| match &saved {
                            Some(uuid) => network.start(uuid.clone()),
                            None => network.join(index),
                        })
                        .fill_width();
                    }
                }
            }
            .gap(Space::TIGHT)
            .fill_width();
        }
        .gap(Space::NORMAL)
        .fill_width(),
    )
}

fn actions(device: &Device) -> impl View {
    row! {
        button(
            row! {
                icon(MaterialSymbol::WIFI_FIND).extent(Length::px(18.0));
                text(tr!("settings.network.wireless.hidden"));
            }
            .gap(Space::SNUG)
            .align_items(Align::Center),
        )
        .secondary()
        .on_press(|network: &mut Network| network.hidden());
        if device
            .wireless
            .as_ref()
            .is_some_and(|wireless| wireless.hotspot_capable)
        {
            button(
                row! {
                    icon(MaterialSymbol::WIFI_TETHERING).extent(Length::px(18.0));
                    text(tr!("settings.network.wireless.hotspot"));
                }
                .gap(Space::SNUG)
                .align_items(Align::Center),
            )
            .secondary()
            .on_press(|network: &mut Network| network.hotspot());
        }
        spacer();
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}
