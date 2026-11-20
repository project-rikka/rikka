use std::cell::Cell;
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub passes: u64,
    pub frames: u64,
    pub views: u64,
    pub builds: u64,
    pub elements: u64,
    pub components: u64,
    pub retained: u64,
    pub reconciled: u64,
    pub painted: u64,
    pub visited: u64,
    pub layouts: u64,
    pub layout_time: Duration,
    pub measures: u64,
    pub woken_by: Reasons,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reasons {
    pub view: u64,
    pub build: u64,
    pub wake: u64,
    pub animation: u64,
    pub repaint: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reason {
    View,
    Build,
    Wake,
    Animation,
    Repaint,
}

impl Stats {
    #[must_use]
    pub fn since(self, earlier: Stats) -> Stats {
        Stats {
            passes: self.passes - earlier.passes,
            frames: self.frames - earlier.frames,
            views: self.views - earlier.views,
            builds: self.builds - earlier.builds,
            elements: self.elements - earlier.elements,
            components: self.components - earlier.components,
            retained: self.retained - earlier.retained,
            reconciled: self.reconciled - earlier.reconciled,
            painted: self.painted - earlier.painted,
            visited: self.visited - earlier.visited,
            layouts: self.layouts - earlier.layouts,
            layout_time: self.layout_time.saturating_sub(earlier.layout_time),
            measures: self.measures - earlier.measures,
            woken_by: Reasons {
                view: self.woken_by.view - earlier.woken_by.view,
                build: self.woken_by.build - earlier.woken_by.build,
                wake: self.woken_by.wake - earlier.woken_by.wake,
                animation: self.woken_by.animation - earlier.woken_by.animation,
                repaint: self.woken_by.repaint - earlier.woken_by.repaint,
            },
        }
    }
}

#[derive(Default)]
struct Counters {
    passes: Cell<u64>,
    views: Cell<u64>,
    builds: Cell<u64>,
    elements: Cell<u64>,
    reconciled: Cell<u64>,
    painted: Cell<u64>,
    visited: Cell<u64>,
    layouts: Cell<u64>,
    layout_time: Cell<Duration>,
    measures: Cell<u64>,
    reasons: [Cell<u64>; 5],
    components: Cell<u64>,
    retained: Cell<u64>,
    muted: Cell<u64>,
}

thread_local! {
    static COUNTERS: Counters = Counters::default();
}

#[must_use]
pub fn snapshot() -> Stats {
    COUNTERS.with(|counters| {
        let reasons = |reason: Reason| counters.reasons[reason as usize].get();
        let woken_by = Reasons {
            view: reasons(Reason::View),
            build: reasons(Reason::Build),
            wake: reasons(Reason::Wake),
            animation: reasons(Reason::Animation),
            repaint: reasons(Reason::Repaint),
        };
        Stats {
            passes: counters.passes.get(),
            frames: counters.reasons.iter().map(Cell::get).sum(),
            views: counters.views.get(),
            builds: counters.builds.get(),
            elements: counters.elements.get(),
            components: counters.components.get(),
            retained: counters.retained.get(),
            reconciled: counters.reconciled.get(),
            painted: counters.painted.get(),
            visited: counters.visited.get(),
            layouts: counters.layouts.get(),
            layout_time: counters.layout_time.get(),
            measures: counters.measures.get(),
            woken_by,
        }
    })
}

fn bump(counter: impl FnOnce(&Counters) -> &Cell<u64>) {
    COUNTERS.with(|counters| {
        if counters.muted.get() > 0 {
            return;
        }
        let cell = counter(counters);
        cell.set(cell.get() + 1);
    });
}

pub(crate) struct MutedGuard(());

impl MutedGuard {
    pub(crate) fn enter() -> MutedGuard {
        COUNTERS.with(|counters| counters.muted.set(counters.muted.get() + 1));
        MutedGuard(())
    }
}

impl Drop for MutedGuard {
    fn drop(&mut self) {
        COUNTERS.with(|counters| counters.muted.set(counters.muted.get() - 1));
    }
}

pub(crate) fn component_viewed() {
    bump(|counters| &counters.components);
}

pub(crate) fn retained() {
    bump(|counters| &counters.retained);
}

pub(crate) fn passed() {
    bump(|counters| &counters.passes);
}

pub(crate) fn framed(reason: Reason) {
    bump(|counters| &counters.reasons[reason as usize]);
}

pub(crate) fn viewed() {
    bump(|counters| &counters.views);
}

pub(crate) fn built() {
    bump(|counters| &counters.builds);
}

pub(crate) fn element_made() {
    bump(|counters| &counters.elements);
}

pub(crate) fn reconciled() {
    bump(|counters| &counters.reconciled);
}

pub(crate) fn painted() {
    bump(|counters| &counters.painted);
}

pub(crate) fn laid_out(took: Duration) {
    COUNTERS.with(|counters| {
        counters.layouts.set(counters.layouts.get() + 1);
        counters.layout_time.set(counters.layout_time.get() + took);
    });
}

pub(crate) fn measure_called() {
    bump(|counters| &counters.measures);
}

pub(crate) fn visited() {
    bump(|counters| &counters.visited);
}
