use rikka_app::prelude::*;
use rikka_format::format_bytes;
use rikka_widgets::column;

use super::{Monitor, MonitorData};
use crate::format;
use crate::performance::cores::Cores;
use crate::performance::history::History;
use crate::performance::{pane, readout};

const COLOUR: Color = Color::rgb(0.10, 0.53, 0.82);

#[derive(State, Debug, Default)]
pub(crate) struct CpuView {
    logical: Tracked<bool>,
}

pub(crate) struct Cpu<'a> {
    data: MonitorData<'a>,
    logical: bool,
}

impl<'a> Cpu<'a> {
    pub(crate) fn new(data: MonitorData<'a>, view: &CpuView) -> Self {
        Self {
            data,
            logical: *view.logical,
        }
    }

    fn graph(&self) -> Element {
        if !self.logical {
            return pane::percentage(self.data.samples(), COLOUR, tr!("tasks.cpu.graph")).build();
        }
        column! {
            row! {
                text(tr!("tasks.cpu.cores-graph")).caption().muted();
                spacer();
                text(tr!("tasks.graph.full")).caption().muted();
            };
            Element::new(Cores {
                series: self
                    .data
                    .series
                    .map(|series| series.cores.iter().map(History::samples).collect())
                    .unwrap_or_default(),
                colour: COLOUR,
            })
            .fill_width()
            .grow(1.0);
        }
        .gap(Space::TIGHT)
        .grow(1.0)
        .build()
    }
}

impl Monitor for Cpu<'_> {
    fn colour(&self) -> Color {
        COLOUR
    }

    fn title(&self) -> String {
        tr!("tasks.resource.cpu").to_string()
    }

    fn summary(&self) -> String {
        let cpu = &self.data.reading.machine.cpu;
        tr!(
            "tasks.card.cpu",
            usage = &format::percent(cpu.usage, 0),
            speed = &format::ghz(cpu.ghz),
        )
        .to_string()
    }

    fn pane(&self) -> Element {
        let cpu = &self.data.reading.machine.cpu;
        let topology = &self.data.reading.topology;
        column! {
            pane::header(self.title(), topology.model.clone());
            segmented(usize::from(self.logical))
                .segments([tr!("tasks.cpu.overall"), tr!("tasks.cpu.logical")])
                .control_size(ControlSize::Small)
                .on_select(|view: &mut CpuView, index: usize| {
                    view.logical.set(index == 1);
                });
            page_stack(usize::from(self.logical), self.graph())
                .grow(1.0)
                .fill_width();
            pane::footer(
                readout::stats(vec![
                    (tr!("tasks.cpu.usage"), format::percent(cpu.usage, 0)),
                    (tr!("tasks.cpu.speed"), format::ghz(cpu.ghz)),
                    (tr!("tasks.cpu.processes"), cpu.processes.to_string()),
                    (tr!("tasks.cpu.threads"), cpu.threads.to_string()),
                    (tr!("tasks.cpu.handles"), cpu.handles.to_string()),
                    (tr!("tasks.cpu.uptime"), format::uptime(cpu.uptime)),
                ]),
                readout::details(vec![
                    (tr!("tasks.cpu.base-speed"), format::ghz(topology.base_ghz)),
                    (tr!("tasks.cpu.sockets"), topology.sockets.to_string()),
                    (tr!("tasks.cpu.physical"), topology.cores.to_string()),
                    (tr!("tasks.cpu.logical-count"), topology.logical.to_string()),
                    (
                        tr!("tasks.cpu.virtualization"),
                        if topology.virtualization {
                            tr!("tasks.enabled")
                        } else {
                            tr!("tasks.disabled")
                        }
                        .to_string(),
                    ),
                    (tr!("tasks.cpu.l1"), format_bytes(topology.caches.l1)),
                    (tr!("tasks.cpu.l2"), format_bytes(topology.caches.l2)),
                    (tr!("tasks.cpu.l3"), format_bytes(topology.caches.l3)),
                ]),
            );
        }
        .gap(Space::NORMAL)
        .fill()
        .build()
    }
}
