use std::rc::Rc;

use desk_layout::{Content, Meter, Monitor};
use rikka_ui::{Element, Space, Styled, View};
use rikka_widgets::{column_of, field, switch};

use crate::widgets::monitor::named;
use crate::widgets::options::Change;

const METERS: [Meter; 5] = [
    Meter::Cpu,
    Meter::Memory,
    Meter::Gpu,
    Meter::Network,
    Meter::Disk,
];

#[must_use]
pub(crate) fn options(monitor: &Monitor, change: &Change) -> Element {
    let lonely = monitor.meters.len() <= 1;
    column_of(METERS.into_iter().map(|meter| {
        let on = monitor.meters.contains(&meter);
        let (change, chosen) = (Rc::clone(change), monitor.meters.clone());
        field(
            named(meter),
            switch(on)
                .enabled(!(on && lonely))
                .on_toggle(move |on: bool| {
                    let meters = METERS
                        .into_iter()
                        .filter(|current| match *current == meter {
                            true => on,
                            false => chosen.contains(current),
                        })
                        .collect();
                    change(Content::Monitor(Monitor { meters }));
                }),
        )
        .build()
    }))
    .gap(Space::NORMAL)
    .fill_width()
    .build()
}
