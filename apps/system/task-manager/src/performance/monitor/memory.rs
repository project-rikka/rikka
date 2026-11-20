use rikka_app::prelude::*;
use rikka_format::format_bytes;
use rikka_widgets::column;

use super::{Monitor, MonitorData};
use crate::format;
use crate::performance::pane::{self, Plot};
use crate::performance::readout;

const COLOUR: Color = Color::rgb(0.47, 0.35, 0.75);

pub(crate) struct Memory<'a> {
    data: MonitorData<'a>,
}

impl<'a> Memory<'a> {
    pub(crate) fn new(data: MonitorData<'a>) -> Self {
        Self { data }
    }
}

impl Monitor for Memory<'_> {
    fn colour(&self) -> Color {
        COLOUR
    }

    fn title(&self) -> String {
        tr!("tasks.resource.memory").to_string()
    }

    fn summary(&self) -> String {
        let memory = &self.data.reading.machine.memory;
        tr!(
            "tasks.card.memory",
            used = &format_bytes(memory.used),
            total = &format_bytes(memory.total),
            usage = &format::percent(memory.fraction(), 0),
        )
        .to_string()
    }

    fn scale(&self) -> f32 {
        self.data.reading.machine.memory.total.max(1) as f32
    }

    fn pane(&self) -> Element {
        let memory = &self.data.reading.machine.memory;
        let free = memory
            .total
            .saturating_sub(memory.used)
            .saturating_sub(memory.cached.min(memory.total.saturating_sub(memory.used)));
        let cached = memory
            .total
            .saturating_sub(memory.used)
            .saturating_sub(free);

        column! {
            pane::header(self.title(), format_bytes(memory.total));
            pane::plot(
                Plot {
                    samples: self.data.samples(),
                    second: None,
                    scale: self.scale(),
                    colour: COLOUR,
                    ceiling: format_bytes(memory.total),
                    caption: tr!("tasks.memory.graph"),
                },
                1.0,
            );
            text(tr!("tasks.memory.composition")).caption().muted();
            readout::composition(&[
                (memory.used as f32, COLOUR),
                (cached as f32, COLOUR.with_alpha(0.45)),
                (free as f32, palette().surface_variant),
            ]);
            pane::footer(
                readout::stats(vec![
                    (tr!("tasks.memory.in-use"), format_bytes(memory.used)),
                    (tr!("tasks.memory.available"), format_bytes(memory.available)),
                    (
                        tr!("tasks.memory.committed"),
                        tr!(
                            "tasks.memory.committed-value",
                            used = &format_bytes(memory.committed),
                            limit = &format_bytes(memory.commit_limit),
                        )
                        .to_string(),
                    ),
                    (tr!("tasks.memory.cached"), format_bytes(memory.cached)),
                    (tr!("tasks.memory.paged"), format_bytes(memory.paged_pool)),
                    (tr!("tasks.memory.nonpaged"), format_bytes(memory.nonpaged_pool)),
                ]),
                readout::details(vec![
                    (tr!("tasks.memory.total"), format_bytes(memory.total)),
                    (
                        tr!("tasks.memory.swap"),
                        tr!(
                            "tasks.memory.committed-value",
                            used = &format_bytes(memory.swap_used),
                            limit = &format_bytes(memory.swap_total),
                        )
                        .to_string(),
                    ),
                ]),
            );
        }
        .gap(Space::NORMAL)
        .fill()
        .build()
    }
}
