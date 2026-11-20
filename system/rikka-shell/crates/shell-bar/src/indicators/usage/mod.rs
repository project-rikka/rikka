mod mark;
mod panel;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use rikka_llm_accounts::{AccountId, AccountsChanged, Llm, Reading, Readings, UsageReport};
use rikka_seam::Delivery;
use rikka_ui::{Element, View, spawn};
use rikka_worker::Observable;

use crate::indicators::{Indicator, Kind, Shelf};

const STALE: Duration = Duration::from_secs(5 * 60);

const FRESH: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub(crate) struct Usage(Rc<Inner>);

struct Inner {
    llm: RefCell<Llm>,
    readings: RefCell<Readings>,
    asked_at: Cell<Option<Instant>>,
    changed: Observable<()>,
}

#[derive(PartialEq)]
pub(crate) struct UsageSource {
    pub(crate) account: AccountId,
    pub(crate) label: String,
    pub(crate) who: String,
}

impl Usage {
    #[must_use]
    pub(crate) fn load() -> Usage {
        Usage(Rc::new(Inner {
            llm: RefCell::new(Llm::load()),
            readings: RefCell::default(),
            asked_at: Cell::new(None),
            changed: Observable::default(),
        }))
    }

    pub(crate) fn event(&self, event: &Delivery) {
        let Some(announced) = AccountsChanged::from_delivery(event) else {
            return;
        };
        let before = self.metered();
        self.0.llm.borrow_mut().apply_change(announced);
        let after = self.metered();
        if after == before {
            return;
        }
        self.0
            .readings
            .borrow_mut()
            .retain(|account| after.iter().any(|metered| metered.account == account));
        self.0.asked_at.set(None);
        self.0.changed.set(());
    }

    pub(crate) fn refresh(&self) {
        self.0.asked_at.set(Some(Instant::now()));
        let gauging = self.0.readings.borrow_mut().request(&self.0.llm.borrow());
        for gauge in gauging {
            let usage = self.clone();
            spawn(gauge.run(), move |gauged: UsageReport| {
                usage.answered(gauged)
            });
        }
        self.0.changed.set(());
    }

    fn answered(&self, gauged: UsageReport) {
        if self.0.readings.borrow_mut().record(gauged) {
            self.0.changed.set(());
        }
    }

    fn older_than(&self, age: Duration) -> bool {
        self.0.asked_at.get().is_none_or(|at| at.elapsed() >= age)
    }

    fn has_metered(&self) -> bool {
        self.0.llm.borrow().metered().next().is_some()
    }

    pub(crate) fn metered(&self) -> Vec<UsageSource> {
        self.0
            .llm
            .borrow()
            .metered()
            .map(|account| UsageSource {
                account: account.id(),
                label: account.label().to_owned(),
                who: account.identity(),
            })
            .collect()
    }

    fn with_reading<T>(&self, account: AccountId, read: impl FnOnce(Option<&Reading>) -> T) -> T {
        read(self.0.readings.borrow().get(account))
    }
}

impl Indicator for Usage {
    fn kind(&self) -> Kind {
        Kind::AiUsage
    }

    fn changes(&self) -> BoxStream<'static, ()> {
        self.0.changed.changes().boxed()
    }

    fn mark(&self) -> Option<Element> {
        if !self.has_metered() {
            return None;
        }
        Some(mark::symbol().build())
    }

    fn panel(&self, shelf: &Shelf) -> Element {
        panel::view(self, shelf).build()
    }

    fn on_open(&self) {
        if self.older_than(FRESH) {
            self.refresh();
        }
    }

    fn tick(&self) {
        if self.has_metered() && self.older_than(STALE) {
            self.refresh();
        }
    }
}

impl std::fmt::Debug for Usage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Usage")
            .field("readings", &*self.0.readings.borrow())
            .finish_non_exhaustive()
    }
}
