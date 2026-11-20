use std::cell::RefCell;

use crate::view::observe::log;

#[derive(Default)]
struct Recording {
    keys: Vec<u64>,
    since: Option<u64>,
}

thread_local! {
    static READS: RefCell<Vec<Recording>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn read(key: u64) {
    READS.with(|reads| {
        if let Some(open) = reads.borrow_mut().last_mut() {
            open.keys.push(key);
        }
    });
}

pub(crate) fn recording() -> bool {
    READS.with(|reads| !reads.borrow().is_empty())
}

pub(crate) fn recorded<R>(body: impl FnOnce() -> R) -> (R, Vec<u64>) {
    let (result, keys, _) = dated(body);
    (result, keys)
}

pub(crate) fn dated<R>(body: impl FnOnce() -> R) -> (R, Vec<u64>, Option<u64>) {
    READS.with(|reads| reads.borrow_mut().push(Recording::default()));
    let open = Open;
    let result = body();
    std::mem::forget(open);
    let Recording { mut keys, since } = READS
        .with(|reads| reads.borrow_mut().pop())
        .unwrap_or_default();
    keys.sort_unstable();
    keys.dedup();
    (result, keys, since)
}

#[derive(Debug)]
pub struct Reads {
    keys: Vec<u64>,
    since: u64,
}

impl Reads {
    pub fn record<R>(build: impl FnOnce() -> R) -> (R, Reads) {
        let since = log::now();
        let (built, keys, earlier) = dated(build);
        let since = earlier.map_or(since, |earlier| earlier.min(since));
        (built, Reads { keys, since })
    }

    pub fn replay(&self) {
        READS.with(|reads| {
            if let Some(open) = reads.borrow_mut().last_mut() {
                open.keys.extend_from_slice(&self.keys);
                open.since = Some(open.since.map_or(self.since, |since| since.min(self.since)));
            }
        });
    }
}

struct Open;

impl Drop for Open {
    fn drop(&mut self) {
        READS.with(|reads| reads.borrow_mut().pop());
    }
}
