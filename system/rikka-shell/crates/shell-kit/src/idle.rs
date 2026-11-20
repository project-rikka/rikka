use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use crate::Tracked;
use crate::host::Host;

const MINUTE: u64 = 60;

type IdleCallback = Rc<dyn Fn(&mut Host)>;

#[must_use]
pub fn idle_minutes(count: i64) -> Option<Duration> {
    u64::try_from(count)
        .ok()
        .filter(|count| *count > 0)
        .map(|count| Duration::from_secs(count * MINUTE))
}

#[derive(Clone, Default)]
pub struct Idle {
    after: Tracked<Option<Duration>>,
    idled: Rc<RefCell<Vec<IdleCallback>>>,
    resumed: Rc<RefCell<Vec<IdleCallback>>>,
}

#[derive(Clone, Default)]
pub struct Idles(Rc<RefCell<Vec<Idle>>>);

impl Idle {
    pub fn after(&self, after: Option<Duration>) {
        self.after.set(after.filter(|after| !after.is_zero()));
    }

    pub fn on_idle(&self, event: impl Fn(&mut Host) + 'static) {
        self.idled.borrow_mut().push(Rc::new(event));
    }

    pub fn on_resume(&self, event: impl Fn(&mut Host) + 'static) {
        self.resumed.borrow_mut().push(Rc::new(event));
    }

    pub fn take_moved(&self) -> bool {
        self.after.take_moved()
    }

    pub fn timeout(&self) -> Option<Duration> {
        self.after.get()
    }

    pub fn idled(&self, host: &mut Host) {
        run(&self.idled, host);
    }

    pub fn resumed(&self, host: &mut Host) {
        run(&self.resumed, host);
    }
}

impl Idles {
    pub(super) fn add(&self) -> Idle {
        let idle = Idle::default();
        self.0.borrow_mut().push(idle.clone());
        idle
    }

    pub fn get(&self, index: usize) -> Option<Idle> {
        self.0.borrow().get(index).cloned()
    }
}

fn run(callbacks: &RefCell<Vec<IdleCallback>>, host: &mut Host) {
    let callbacks: Vec<IdleCallback> = callbacks.borrow().clone();
    for callback in callbacks {
        callback(host);
    }
}

impl std::fmt::Debug for Idle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Idle")
            .field("after", &self.after.get())
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Idles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Idles")
            .field("timers", &self.0.borrow().len())
            .finish()
    }
}
