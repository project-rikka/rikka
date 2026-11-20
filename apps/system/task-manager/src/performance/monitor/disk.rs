use rikka_app::prelude::*;
use rikka_format::format_bytes;
use rikka_sysmon::{DiskKind, DiskSample};
use rikka_widgets::column;

use super::{Monitor, MonitorData, RATE_FLOOR};
use crate::format;
use crate::performance::pane::{self, Plot};
use crate::performance::readout;

const COLOUR: Color = Color::rgb(0.16, 0.56, 0.40);

pub(crate) struct Disk<'a> {
    data: MonitorData<'a>,
    disk: &'a DiskSample,
    index: usize,
}

impl<'a> Disk<'a> {
    pub(crate) fn find(data: MonitorData<'a>, name: &str) -> Option<Self> {
        let disks = &data.reading.machine.disks;
        let index = disks.iter().position(|disk| disk.name == name)?;
        Some(Self {
            data,
            disk: &disks[index],
            index,
        })
    }
}

impl Monitor for Disk<'_> {
    fn colour(&self) -> Color {
        COLOUR
    }

    fn title(&self) -> String {
        tr!(
            "tasks.resource.disk",
            index = self.index,
            name = self.disk.name.as_str(),
        )
        .to_string()
    }

    fn summary(&self) -> String {
        format::percent(self.disk.active, 0)
    }

    fn pane(&self) -> Element {
        let disk = self.disk;
        let transfer = self.data.ceiling(RATE_FLOOR);

        column! {
            pane::header(self.title(), disk.model.clone());
            pane::percentage(self.data.samples(), COLOUR, tr!("tasks.disk.graph"));
            pane::plot(
                Plot {
                    samples: self.data.secondary(),
                    second: None,
                    scale: transfer,
                    colour: COLOUR,
                    ceiling: format::rate(transfer),
                    caption: tr!("tasks.disk.transfer"),
                },
                0.7,
            );
            pane::footer(
                readout::stats(vec![
                    (tr!("tasks.disk.active"), format::percent(disk.active, 0)),
                    (
                        tr!("tasks.disk.response"),
                        tr!(
                            "units.milliseconds",
                            value = &format!("{:.1}", disk.response_ms)
                        )
                        .to_string(),
                    ),
                    (tr!("tasks.disk.read"), format::rate(disk.read)),
                    (tr!("tasks.disk.write"), format::rate(disk.write)),
                ]),
                readout::details(vec![
                    (tr!("tasks.disk.capacity"), format_bytes(disk.capacity)),
                    (tr!("tasks.disk.kind"), kind(disk.kind)),
                    (
                        tr!("tasks.disk.system"),
                        if disk.system {
                            tr!("tasks.yes")
                        } else {
                            tr!("tasks.no")
                        }
                        .to_string(),
                    ),
                    (tr!("tasks.disk.queue"), disk.queue.to_string()),
                ]),
            );
        }
        .gap(Space::NORMAL)
        .fill()
        .build()
    }
}

fn kind(kind: DiskKind) -> String {
    match kind {
        DiskKind::Nvme => tr!("tasks.disk.nvme"),
        DiskKind::Ssd => tr!("tasks.disk.ssd"),
        DiskKind::Hdd => tr!("tasks.disk.hdd"),
    }
    .to_string()
}
