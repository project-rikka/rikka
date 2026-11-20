use rikka_sync::{Mutex, MutexGuard};
use std::sync::{Arc, OnceLock};

use rustc_hash::FxHashMap;

use crate::turn::Body;

const BUDGET: usize = 24 * 1024 * 1024;

const RELIEF: usize = BUDGET / 4;

pub(crate) fn keep(body: &Arc<Body>) {
    let mut cache = shelf();
    let clock = cache.clock + 1;
    cache.clock = clock;
    let at = Arc::as_ptr(body) as usize;
    if let Some(entry) = cache.bodies.get_mut(&at) {
        entry.used = clock;
        return;
    }
    let bytes = body.bytes();
    cache.bodies.insert(
        at,
        ResidentBody {
            body: Arc::clone(body),
            bytes,
            used: clock,
        },
    );
    cache.bytes += bytes;
    make_room(&mut cache);
}

fn make_room(shelf: &mut Shelf) {
    if shelf.bytes <= BUDGET {
        return;
    }
    let mut order: Vec<(bool, u64, usize)> = shelf
        .bodies
        .iter()
        .map(|(at, entry)| (Arc::strong_count(&entry.body) > 1, entry.used, *at))
        .collect();
    order.sort_unstable_by_key(|(in_use, used, _)| (*in_use, *used));
    for (_, _, at) in order {
        if shelf.bytes <= BUDGET - RELIEF {
            return;
        }
        if let Some(gone) = shelf.bodies.remove(&at) {
            shelf.bytes -= gone.bytes;
        }
    }
}

struct ResidentBody {
    body: Arc<Body>,
    bytes: usize,
    used: u64,
}

#[derive(Default)]
struct Shelf {
    bodies: FxHashMap<usize, ResidentBody>,
    bytes: usize,
    clock: u64,
}

fn shelf() -> MutexGuard<'static, Shelf> {
    static SHELF: OnceLock<Mutex<Shelf>> = OnceLock::new();
    SHELF.get_or_init(|| Mutex::new(Shelf::default())).lock()
}
