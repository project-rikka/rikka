use std::cell::Cell;

thread_local! {
    static INSIDE: Cell<bool> = const { Cell::new(false) };
}

pub(crate) fn is_inside() -> bool {
    INSIDE.get()
}

pub(crate) fn inside<R>(work: impl FnOnce() -> R) -> R {
    let _restore = Restore(INSIDE.replace(true));
    work()
}

struct Restore(bool);

impl Drop for Restore {
    fn drop(&mut self) {
        INSIDE.set(self.0);
    }
}
