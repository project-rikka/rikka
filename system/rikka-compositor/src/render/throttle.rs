use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::time::Duration;

use rikka_compositor_reactor::Reactor;
use rikka_compositor_surface::{SurfaceId, Surfaces};

use super::Scenes;
use super::clock::monotonic;
use crate::state::Compositor;

pub(crate) const HIDDEN_FRAME_INTERVAL: Duration = Duration::from_secs(1);

pub(crate) struct Throttle {
    reactor: Reactor<Compositor>,
    waiting: BTreeMap<Duration, BTreeSet<SurfaceId>>,
}

impl Throttle {
    pub(crate) fn new(reactor: &Reactor<Compositor>) -> Self {
        Self {
            reactor: reactor.clone(),
            waiting: BTreeMap::new(),
        }
    }

    pub(crate) fn wait(&mut self, root: SurfaceId, pace: Duration) {
        let queue = self.waiting.entry(pace).or_default();
        let armed = !queue.is_empty();
        queue.insert(root);
        if !armed {
            self.reactor
                .after(pace, move |compositor: &mut Compositor| {
                    let Compositor {
                        throttle,
                        scenes,
                        surfaces,
                        ..
                    } = compositor;
                    throttle.answer_hidden_frames(pace, scenes, surfaces)
                });
        }
    }

    fn answer_hidden_frames(&mut self, pace: Duration, scenes: &Scenes, surfaces: &mut Surfaces) {
        let time = monotonic();
        for root in self.waiting.remove(&pace).unwrap_or_default() {
            if !scenes.drawn_within(root, pace) {
                surfaces.frame_done(root, time);
            }
        }
    }
}

impl fmt::Debug for Throttle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Throttle")
            .field(
                "waiting",
                &self
                    .waiting
                    .iter()
                    .map(|(pace, roots)| (*pace, roots.len()))
                    .collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}
