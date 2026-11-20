use std::rc::Rc;
use std::time::Duration;

use rikka_app::prelude::*;

use crate::performance::{self, Performance};
use crate::processes::{self, Mode, Processes};
use crate::sampler::{Reading, Sampler};
use crate::tab::Tab;

const PERIOD: Duration = Duration::from_secs(1);
const SIDEBAR: f32 = 208.0;

#[derive(State)]
pub(crate) struct Manager {
    #[untracked]
    sampler: Option<Sampler>,
    tab: Tracked<Tab>,
    #[state]
    processes: Processes,
    #[state]
    details: Processes,
    #[state]
    performance: Performance,
    #[untracked]
    sampling: bool,
}

impl Manager {
    pub(crate) fn new() -> Self {
        Self {
            sampler: None,
            tab: Tracked::default(),
            processes: Processes::new(Mode::Grouped),
            details: Processes::new(Mode::Flat),
            performance: Performance::new(),
            sampling: false,
        }
    }

    pub(crate) fn start(&mut self) {
        self.sample();
        every(PERIOD, |manager: &mut Manager| {
            manager.sample();
            skip_view();
        });
    }

    fn sample(&mut self) {
        if self.sampling {
            return;
        }
        self.sampling = true;
        let sampler = self.sampler.take();
        spawn_blocking(
            move || {
                let mut sampler = sampler.unwrap_or_else(Sampler::new);
                let reading = sampler.read();
                (sampler, reading)
            },
            |manager: &mut Manager, (sampler, reading): (Sampler, Reading)| {
                manager.sampler = Some(sampler);
                manager.apply(reading);
            },
        );
    }

    pub(crate) fn focus_search(&mut self) {
        match *self.tab {
            Tab::Processes => self.processes.focus_search(),
            Tab::Details => self.details.focus_search(),
            Tab::Performance => skip_view(),
        }
    }

    fn open(&mut self, tab: Tab) {
        self.tab.set(tab);
        self.rebuild_if_stale();
    }

    fn error(&self) -> Option<&Label> {
        match *self.tab {
            Tab::Processes => self.processes.error(),
            Tab::Details => self.details.error(),
            Tab::Performance => None,
        }
    }

    fn rebuild_if_stale(&mut self) {
        match *self.tab {
            Tab::Processes => self.processes.rebuild_if_stale(),
            Tab::Details => self.details.rebuild_if_stale(),
            Tab::Performance => {}
        }
    }

    fn apply(&mut self, reading: Reading) {
        let reading = Rc::new(reading);
        self.processes.set_reading(Rc::clone(&reading));
        self.details.set_reading(Rc::clone(&reading));
        self.performance.set_reading(reading);
        self.rebuild_if_stale();
        self.sampling = false;
    }
}

pub(crate) fn view(manager: &Manager) -> impl View {
    let tab = *manager.tab;
    row! {
        lazy(tab.index(), move || sidebar(tab)).fill_height();
        page_stack(tab.index(), body(manager))
            .grow(1.0)
            .fill_height();
    }
    .fill()
}

fn sidebar(tab: Tab) -> impl View {
    row! {
        container(
            nav_list(tab.index())
                .items(Tab::ALL.map(Tab::nav_item))
                .on_select(|manager: &mut Manager, index: usize| manager.open(Tab::ALL[index]))
                .fill_width(),
        )
        .padding(Inset::xy(8.0, 12.0))
        .width(SIDEBAR)
        .fill_height();
        rule_upright();
    }
    .fill_height()
}

fn body(manager: &Manager) -> Element {
    match *manager.tab {
        Tab::Processes => scope(
            |manager: &mut Manager| &mut manager.processes,
            component(|manager: &Manager| &manager.processes, processes::view),
        ),
        Tab::Performance => component(|manager: &Manager| &manager.performance, performance::view),
        Tab::Details => scope(
            |manager: &mut Manager| &mut manager.details,
            component(|manager: &Manager| &manager.details, processes::view),
        ),
    }
}

pub(crate) fn caption_bar(manager: &Manager) -> Option<CaptionBar> {
    let mut bar = caption().leading(
        row! {
            icon(MaterialSymbol::MONITOR_HEART).extent(Length::px(16.0));
            text(tr!("tasks.title")).label().medium().no_wrap();
        }
        .gap(Space::SNUG)
        .align_items(Align::Center),
    );
    bar = match *manager.tab {
        Tab::Processes => bar.center(scope(
            |manager: &mut Manager| &mut manager.processes,
            processes::search_field(&manager.processes),
        )),
        Tab::Details => bar.center(scope(
            |manager: &mut Manager| &mut manager.details,
            processes::search_field(&manager.details),
        )),
        Tab::Performance => bar,
    };
    if let Some(error) = manager.error() {
        bar = bar.trailing(
            row! {
                icon(MaterialSymbol::ERROR)
                    .extent(Length::px(15.0))
                    .color(palette().danger);
                text(error.clone()).caption().no_wrap();
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center),
        );
    }
    Some(bar.into())
}

impl std::fmt::Debug for Manager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Manager")
            .field("tab", &self.tab)
            .field("sampling", &self.sampling)
            .finish_non_exhaustive()
    }
}
