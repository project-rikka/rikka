use std::rc::Rc;

use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::performance::graph::Graph;
use crate::performance::monitor::{Cpu, CpuView, Disk, Gpu, Memory, Monitor, MonitorData, Network};
use crate::performance::resource::Resource;
use crate::performance::series::Charts;
use crate::sampler::Reading;

const LIST: f32 = 232.0;
const CARD: f32 = 72.0;
const MINI: f32 = 46.0;
const PANE: f32 = 20.0;

#[derive(State)]
pub(crate) struct Performance {
    charts: Tracked<Charts>,
    reading: Tracked<Rc<Reading>>,
    open: Tracked<Resource>,
    #[state]
    cpu: CpuView,
}

pub(crate) fn view(performance: &Performance) -> impl View {
    row! {
        performance.list();
        performance.pane();
    }
    .fill()
}

impl Performance {
    pub(crate) fn new() -> Self {
        Self {
            charts: Tracked::default(),
            reading: Tracked::new(Rc::new(Reading::default())),
            open: Tracked::new(Resource::Cpu),
            cpu: CpuView::default(),
        }
    }

    pub(crate) fn set_reading(&mut self, reading: Rc<Reading>) {
        self.charts.record(&reading);
        *self.reading = reading;
        if !self.charts.order().contains(&*self.open) {
            self.open.set(Resource::Cpu);
        }
    }

    fn data(&self, resource: &Resource) -> MonitorData<'_> {
        MonitorData::new(&self.reading, self.charts.get(resource))
    }

    fn monitor(&self, resource: &Resource) -> Option<Box<dyn Monitor + '_>> {
        let data = self.data(resource);
        Some(match resource {
            Resource::Cpu => Box::new(Cpu::new(data, &self.cpu)),
            Resource::Memory => Box::new(Memory::new(data)),
            Resource::Disk(name) => Box::new(Disk::find(data, name)?),
            Resource::Network(name) => Box::new(Network::find(data, name)?),
            Resource::Gpu(slot) => Box::new(Gpu::find(data, slot)?),
        })
    }

    fn list(&self) -> impl View {
        row! {
            scrollable(
                column! {
                    for resource in self.charts.order() {
                        self.card(resource);
                    }
                }
                .gap(Space::TIGHT)
                .padding(Space::SNUG),
            )
            .width(LIST)
            .fill_height();
            rule_upright();
        }
    }

    fn card(&self, resource: &Resource) -> Option<impl View> {
        let monitor = self.monitor(resource)?;
        let graph = Element::new(Graph {
            samples: self.data(resource).samples(),
            second: None,
            scale: monitor.scale(),
            colour: monitor.colour(),
            ruled: false,
        })
        .width(MINI * 1.6)
        .height(MINI);

        let opened = resource.clone();
        let card = container(
            row! {
                graph;
                column! {
                    text(monitor.title()).label().no_wrap();
                    text(monitor.summary()).caption().muted().no_wrap();
                }
                .gap(Space::HAIR)
                .grow(1.0);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center),
        )
        .padding(Inset::xy(8.0, 6.0))
        .radius(Corner::Item)
        .height(CARD)
        .on_press(move |performance: &mut Performance| performance.open.set(opened.clone()));

        Some(if *resource == *self.open {
            card.background(palette().selection)
        } else {
            card
        })
    }

    fn pane(&self) -> impl View {
        let body = self
            .monitor(&self.open)
            .map(|monitor| monitor.pane())
            .unwrap_or_else(|| spacer().build());
        let at = self
            .charts
            .order()
            .iter()
            .position(|resource| *resource == *self.open)
            .unwrap_or(0);
        container(page_stack(at, body).fill_width().fill_height().grow(1.0))
            .padding(PANE)
            .fill_width()
            .fill_height()
            .grow(1.0)
    }
}

impl std::fmt::Debug for Performance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Performance")
            .field("open", &self.open)
            .field("cpu", &self.cpu)
            .finish_non_exhaustive()
    }
}
