use rikka_app::prelude::*;
use rikka_sysmon::{NetworkKind, NetworkSample};
use rikka_widgets::column;

use super::{Monitor, MonitorData, RATE_FLOOR};
use crate::format;
use crate::format::UNKNOWN;
use crate::performance::pane::{self, Plot};
use crate::performance::readout;

const COLOUR: Color = Color::rgb(0.62, 0.31, 0.60);

pub(crate) struct Network<'a> {
    data: MonitorData<'a>,
    interface: &'a NetworkSample,
}

impl<'a> Network<'a> {
    pub(crate) fn find(data: MonitorData<'a>, name: &str) -> Option<Self> {
        let interface = data
            .reading
            .machine
            .networks
            .iter()
            .find(|interface| interface.name == name)?;
        Some(Self { data, interface })
    }
}

impl Monitor for Network<'_> {
    fn colour(&self) -> Color {
        COLOUR
    }

    fn title(&self) -> String {
        link(self.interface.kind).to_string()
    }

    fn summary(&self) -> String {
        tr!(
            "tasks.card.network",
            send = &format::bits(self.interface.send),
            receive = &format::bits(self.interface.receive),
        )
        .to_string()
    }

    fn scale(&self) -> f32 {
        self.data.ceiling(RATE_FLOOR / 8.0)
    }

    fn pane(&self) -> Element {
        let interface = self.interface;
        let ceiling = self.scale();

        column! {
            pane::header(self.title(), interface.name.clone());
            pane::plot(
                Plot {
                    samples: self.data.samples(),
                    second: Some(self.data.secondary()),
                    scale: ceiling,
                    colour: COLOUR,
                    ceiling: format::bits(ceiling),
                    caption: tr!("tasks.network.graph"),
                },
                1.0,
            );
            pane::footer(
                readout::stats(vec![
                    (tr!("tasks.network.send"), format::bits(interface.send)),
                    (tr!("tasks.network.receive"), format::bits(interface.receive)),
                ]),
                readout::details(vec![
                    (tr!("tasks.network.adapter"), interface.name.clone()),
                    (tr!("tasks.network.kind"), self.title()),
                    (
                        tr!("tasks.network.state"),
                        if interface.up {
                            tr!("tasks.network.connected")
                        } else {
                            tr!("tasks.network.disconnected")
                        }
                        .to_string(),
                    ),
                    (
                        tr!("tasks.network.speed"),
                        interface
                            .speed_mbps
                            .map(|speed| {
                                tr!("units.megabits", value = &speed.to_string()).to_string()
                            })
                            .unwrap_or_else(|| UNKNOWN.to_owned()),
                    ),
                    (
                        tr!("tasks.network.signal"),
                        interface
                            .signal
                            .map(|signal| format::percent(signal, 0))
                            .unwrap_or_else(|| UNKNOWN.to_owned()),
                    ),
                    (tr!("tasks.network.ipv4"), addresses(&interface.link.ipv4)),
                    (tr!("tasks.network.ipv6"), addresses(&interface.link.ipv6)),
                    (tr!("tasks.network.mac"), interface.link.mac.clone()),
                ]),
            );
        }
        .gap(Space::NORMAL)
        .fill()
        .build()
    }
}

fn link(kind: NetworkKind) -> Label {
    match kind {
        NetworkKind::Ethernet => tr!("tasks.network.ethernet"),
        NetworkKind::WiFi => tr!("tasks.network.wifi"),
        NetworkKind::Loopback => tr!("tasks.network.loopback"),
        NetworkKind::Virtual => tr!("tasks.network.virtual"),
    }
}

fn addresses(addresses: &[impl std::fmt::Display]) -> String {
    if addresses.is_empty() {
        return UNKNOWN.to_owned();
    }
    addresses
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<String>>()
        .join(", ")
}
