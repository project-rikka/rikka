use std::cell::RefCell;
use std::sync::Arc;

use crate::text_layout::TextLayout;

const WAYS: usize = 4;

const SETS: usize = 2048;

const LARGEST_KEPT: usize = 1024;

type Slot = Option<(u64, Arc<TextLayout>)>;

#[derive(Clone, Default)]
#[repr(align(64))]
struct Set([Slot; WAYS]);

thread_local! {
    static FRONT: RefCell<Vec<Set>> = RefCell::new(vec![Set::default(); SETS]);
}

pub(crate) fn recall(
    key: u64,
    serves: impl FnOnce(&TextLayout) -> bool,
) -> Option<Arc<TextLayout>> {
    FRONT.with_borrow_mut(|sets| {
        let Set(set) = &mut sets[set(key)];
        let way = set
            .iter()
            .position(|slot| slot.as_ref().is_some_and(|(kept, _)| *kept == key))?;
        let found = set[way].as_ref().map(|(_, layout)| layout)?;
        if !serves(found) {
            return None;
        }
        let found = Arc::clone(found);
        set[..=way].rotate_right(1);
        Some(found)
    })
}

pub(crate) fn keep(key: u64, layout: &Arc<TextLayout>) {
    if layout.bytes() > LARGEST_KEPT {
        return;
    }
    FRONT.with_borrow_mut(|sets| {
        let Set(set) = &mut sets[set(key)];
        let way = set
            .iter()
            .position(|slot| slot.as_ref().is_some_and(|(kept, _)| *kept == key))
            .unwrap_or(WAYS - 1);
        set[way] = Some((key, Arc::clone(layout)));
        set[..=way].rotate_right(1);
    });
}

fn set(key: u64) -> usize {
    (key ^ (key >> 32)) as usize & (SETS - 1)
}
