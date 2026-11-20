use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::HashSet;
use std::rc::Rc;

use futures_util::stream::{Stream, StreamExt};

use crate::provider::{Activation, Hit, Provider, ProviderInfo, Query, RemoteSearch, Within};
use crate::remote::RemoteProvider;
use crate::usage::Usage;

const RECALLED: usize = 8;
const CANDIDATES: usize = RECALLED * 4;

#[derive(Clone)]
pub(super) struct Providers(Rc<RefCell<Inner>>);

struct Inner {
    entries: Vec<Entry>,
    usage: Usage,
}

struct Entry {
    info: ProviderInfo,
    slot: ProviderSlot,
}

enum ProviderSlot {
    Local(Box<dyn Provider>),
    Remote(RemoteProvider),
}

impl ProviderSlot {
    fn here(&mut self) -> Option<&mut Box<dyn Provider>> {
        match self {
            ProviderSlot::Local(provider) => Some(provider),
            ProviderSlot::Remote(_) => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Ranked {
    pub(super) provider: usize,
    pub(crate) hit: Hit,
}

struct Listing {
    fresh: Vec<Ranked>,
    rest: Vec<Ranked>,
}

#[derive(Clone, Debug)]
pub(crate) struct Reading {
    pub(crate) query: Query,
    pub(super) triggered: Option<usize>,
}

impl Providers {
    #[must_use]
    pub(crate) fn new() -> Providers {
        Providers(Rc::new(RefCell::new(Inner {
            entries: Vec::new(),
            usage: Usage::read(),
        })))
    }
    #[must_use]
    pub(crate) fn with(self, provider: impl Provider) -> Providers {
        let provider: Box<dyn Provider> = Box::new(provider);
        let info = provider.info();
        self.0.borrow_mut().entries.push(Entry {
            info,
            slot: ProviderSlot::Local(provider),
        });
        self
    }

    #[must_use]
    pub(crate) fn away(self, provider: impl RemoteSearch) -> Providers {
        let info = provider.info();
        self.0.borrow_mut().entries.push(Entry {
            info,
            slot: ProviderSlot::Remote(RemoteProvider::spawn(provider)),
        });
        self
    }

    pub(crate) fn info(&self, index: usize) -> Option<ProviderInfo> {
        self.0
            .borrow()
            .entries
            .get(index)
            .map(|entry| entry.info.clone())
    }

    pub(crate) fn within(&self, scope: usize) -> Option<Within> {
        match &self.0.borrow().entries.get(scope)?.slot {
            ProviderSlot::Local(provider) => provider.within(),
            ProviderSlot::Remote(_) => None,
        }
    }

    pub(crate) fn changes(&self) -> impl Stream<Item = ()> + Send + 'static {
        let changes: Vec<_> = self
            .0
            .borrow()
            .entries
            .iter()
            .filter_map(|entry| match &entry.slot {
                ProviderSlot::Remote(remote) => Some(remote.arrivals().boxed()),
                ProviderSlot::Local(provider) => provider.changes(),
            })
            .collect();
        futures_util::stream::select_all(changes)
    }

    fn leave_all_but(&self, except: Option<usize>) {
        for (index, entry) in self.0.borrow_mut().entries.iter_mut().enumerate() {
            if except == Some(index) {
                continue;
            }
            if let Some(provider) = entry.slot.here() {
                provider.leave();
            }
        }
    }

    pub(crate) fn reload(&self) {
        for entry in self.0.borrow_mut().entries.iter_mut() {
            if let Some(provider) = entry.slot.here() {
                provider.reload();
            }
        }
    }

    pub(crate) fn read(&self, text: &str) -> Reading {
        let typed = text.trim_start();
        let triggered = self
            .0
            .borrow()
            .entries
            .iter()
            .enumerate()
            .find_map(|(index, entry)| {
                let prefix = entry.info.prefix()?;
                typed.strip_prefix(prefix).map(|rest| (index, rest))
            });
        match triggered {
            Some((index, rest)) => Reading {
                query: Query::new(rest).forced(),
                triggered: Some(index),
            },
            None => Reading {
                query: Query::new(text),
                triggered: None,
            },
        }
    }

    pub(crate) fn answer(&self, reading: &Reading) -> Vec<Ranked> {
        self.leave_all_but(reading.triggered);
        let searched = self.search(&reading.query, reading.triggered);
        if !reading.query.is_empty() {
            return searched;
        }
        let Listing { fresh, rest } = self.listing(reading.triggered);
        let recalled = self.recall(reading.triggered);
        distinct(
            fresh
                .into_iter()
                .chain(searched)
                .chain(recalled)
                .chain(rest),
        )
    }

    pub(crate) fn search(&self, query: &Query, scope: Option<usize>) -> Vec<Ranked> {
        let mut inner = self.0.borrow_mut();
        let mut ranked: Vec<Ranked> = Vec::new();
        for (index, entry) in inner.entries.iter_mut().enumerate() {
            if scope.is_some_and(|scope| scope != index) {
                continue;
            }
            let hits = match &mut entry.slot {
                ProviderSlot::Local(provider) => provider.search(query),
                ProviderSlot::Remote(remote) => remote.ask(query),
            };
            ranked.extend(hits.into_iter().map(|hit| Ranked {
                provider: index,
                hit,
            }));
        }
        inner.lift(&mut ranked);
        Inner::sort(&mut ranked);
        ranked
    }

    fn listing(&self, scope: Option<usize>) -> Listing {
        let mut inner = self.0.borrow_mut();
        let mut listed: Vec<Ranked> = Vec::new();
        for (index, entry) in inner.entries.iter_mut().enumerate() {
            if scope.is_some_and(|scope| scope != index) {
                continue;
            }
            if let Some(provider) = entry.slot.here() {
                listed.extend(provider.listing().into_iter().map(|hit| Ranked {
                    provider: index,
                    hit,
                }));
            }
        }
        let (mut fresh, rest): (Vec<Ranked>, Vec<Ranked>) = listed
            .into_iter()
            .partition(|ranked| inner.is_fresh(ranked));
        fresh.sort_by_key(|ranked| Reverse(ranked.hit.arrival()));
        Listing { fresh, rest }
    }

    pub(super) fn recall(&self, scope: Option<usize>) -> Vec<Ranked> {
        let mut inner = self.0.borrow_mut();
        let keys: Vec<String> = inner
            .usage
            .best(CANDIDATES)
            .into_iter()
            .map(str::to_owned)
            .collect();

        let mut recalled = Vec::new();
        for key in keys {
            let Some((index, id)) = inner.locate(&key) else {
                continue;
            };
            if scope.is_some_and(|scope| scope != index) {
                continue;
            }
            let recalled_hit = match &mut inner.entries[index].slot {
                ProviderSlot::Local(provider) => provider.recall(&id),
                ProviderSlot::Remote(_) => None,
            };
            if let Some(hit) = recalled_hit {
                recalled.push(Ranked {
                    provider: index,
                    hit,
                });
            }
            if recalled.len() >= RECALLED {
                break;
            }
        }
        recalled
    }

    pub(crate) fn activate(&self, ranked: &Ranked) -> Activation {
        let mut inner = self.0.borrow_mut();
        let Some(entry) = inner.entries.get_mut(ranked.provider) else {
            return Activation::Ignore;
        };
        let activation = match (ranked.hit.completion(), &mut entry.slot) {
            (Some(completion), _) => Activation::Complete(entry.info.typed(completion).into()),
            (None, ProviderSlot::Local(provider)) => provider.activate(&ranked.hit),
            (None, ProviderSlot::Remote(remote)) => remote.activate(&ranked.hit),
        };
        if matches!(activation, Activation::Ignore) {
            return activation;
        }
        if entry.info.is_remembered() {
            let mut key = String::new();
            usage_key(&mut key, entry.info.id(), ranked.hit.id());
            inner.usage.record(&key);
        }
        activation
    }
}

impl Inner {
    fn is_fresh(&self, ranked: &Ranked) -> bool {
        let Some(arrival) = ranked.hit.arrival() else {
            return false;
        };
        let mut key = String::new();
        usage_key(
            &mut key,
            self.entries[ranked.provider].info.id(),
            ranked.hit.id(),
        );
        self.usage
            .last_used(&key)
            .is_none_or(|launched| launched < arrival)
    }

    fn lift(&self, ranked: &mut [Ranked]) {
        let scale = self.usage.scale();
        let mut key = String::new();
        for item in ranked.iter_mut() {
            let Some(entry) = self.entries.get(item.provider) else {
                continue;
            };
            if !entry.info.is_remembered() {
                continue;
            }
            usage_key(&mut key, entry.info.id(), item.hit.id());
            let lift = self.usage.lift(&key, scale);
            if lift > 0.0 {
                item.hit.lift(lift);
            }
        }
    }

    fn sort(ranked: &mut [Ranked]) {
        ranked.sort_by(|a, b| {
            b.hit
                .ranking()
                .cmp(&a.hit.ranking())
                .then(a.provider.cmp(&b.provider))
                .then_with(|| a.hit.title().cmp(b.hit.title()))
        });
    }

    fn locate(&self, key: &str) -> Option<(usize, String)> {
        let (provider, id) = key.split_once('/')?;
        let index = self
            .entries
            .iter()
            .position(|entry| entry.info.id() == provider)?;
        Some((index, id.to_owned()))
    }
}

impl std::fmt::Debug for Providers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Providers")
            .field(
                "providers",
                &self
                    .0
                    .borrow()
                    .entries
                    .iter()
                    .map(|entry| entry.info.id().to_owned())
                    .collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

fn distinct(ranked: impl IntoIterator<Item = Ranked>) -> Vec<Ranked> {
    let mut seen: HashSet<(usize, Box<str>)> = HashSet::new();
    ranked
        .into_iter()
        .filter(|item| seen.insert((item.provider, item.hit.id().into())))
        .collect()
}

fn usage_key(out: &mut String, provider: &str, hit: &str) {
    out.clear();
    out.push_str(provider);
    out.push('/');
    out.push_str(hit);
}
