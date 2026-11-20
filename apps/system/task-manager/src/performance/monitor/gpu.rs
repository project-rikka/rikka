use rikka_app::prelude::*;
use rikka_format::format_bytes;
use rikka_sysmon::gpu::{Adapter, Memory};
use rikka_widgets::column;

use super::{Monitor, MonitorData, RATE_FLOOR};
use crate::format;
use crate::format::UNKNOWN;
use crate::performance::pane::{self, Plot};
use crate::performance::readout;

const COLOUR: Color = Color::rgb(0.78, 0.42, 0.16);

pub(crate) struct Gpu<'a> {
    data: MonitorData<'a>,
    adapter: &'a Adapter,
    index: usize,
}

impl<'a> Gpu<'a> {
    pub(crate) fn find(data: MonitorData<'a>, slot: &str) -> Option<Self> {
        let adapters = &data.reading.adapters;
        let index = adapters.iter().position(|adapter| adapter.slot == slot)?;
        Some(Self {
            data,
            adapter: &adapters[index],
            index,
        })
    }
}

impl Monitor for Gpu<'_> {
    fn colour(&self) -> Color {
        COLOUR
    }

    fn title(&self) -> String {
        tr!("tasks.resource.gpu", index = self.index).to_string()
    }

    fn summary(&self) -> String {
        format::percent(self.adapter.usage, 0)
    }

    fn pane(&self) -> Element {
        let adapter = self.adapter;
        let dedicated = adapter.dedicated;
        let ceiling = dedicated
            .filter(|memory| memory.total > 0)
            .map(|memory| memory.total as f32)
            .unwrap_or_else(|| self.data.ceiling(RATE_FLOOR));

        column! {
            pane::header(self.title(), adapter.name.clone());
            pane::percentage(self.data.samples(), COLOUR, tr!("tasks.gpu.graph"));
            if dedicated.is_some() {
                pane::plot(
                    Plot {
                        samples: self.data.secondary(),
                        second: None,
                        scale: ceiling,
                        colour: COLOUR,
                        ceiling: format_bytes(ceiling as u64),
                        caption: tr!("tasks.gpu.dedicated-graph"),
                    },
                    0.7,
                );
            }
            pane::footer(
                readout::stats(vec![
                    (tr!("tasks.gpu.usage"), format::percent(adapter.usage, 0)),
                    (tr!("tasks.gpu.dedicated"), memory(dedicated)),
                    (tr!("tasks.gpu.shared"), memory(adapter.shared)),
                    (
                        tr!("tasks.gpu.temperature"),
                        adapter
                            .temperature
                            .map(|degrees| {
                                tr!("units.celsius", value = &format!("{degrees:.0}")).to_string()
                            })
                            .unwrap_or_else(|| UNKNOWN.to_owned()),
                    ),
                ]),
                readout::details(vec![
                    (tr!("tasks.gpu.driver"), adapter.driver.clone()),
                    (tr!("tasks.gpu.slot"), adapter.slot.clone()),
                    (tr!("tasks.gpu.encode"), share(adapter.encode)),
                    (tr!("tasks.gpu.decode"), share(adapter.decode)),
                ]),
            );
        }
        .gap(Space::NORMAL)
        .fill()
        .build()
    }
}

fn memory(memory: Option<Memory>) -> String {
    let Some(memory) = memory else {
        return UNKNOWN.to_owned();
    };
    if memory.total == 0 {
        return format_bytes(memory.used);
    }
    tr!(
        "tasks.memory.committed-value",
        used = &format_bytes(memory.used),
        limit = &format_bytes(memory.total),
    )
    .to_string()
}

fn share(share: Option<f32>) -> String {
    share
        .map(|share| format::percent(share, 0))
        .unwrap_or_else(|| UNKNOWN.to_owned())
}
