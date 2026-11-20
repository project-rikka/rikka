use std::cell::Cell;
use std::fmt::Display;

thread_local! {
    static ENTRY: Cell<&'static str> = const { Cell::new("rikka") };
}

#[derive(Debug)]
pub(crate) struct Entry {
    previous: &'static str,
}

pub(crate) fn enter(function: &'static str) -> Entry {
    Entry {
        previous: ENTRY.replace(function),
    }
}

impl Drop for Entry {
    fn drop(&mut self) {
        ENTRY.set(self.previous);
    }
}

pub(crate) fn violation(message: impl Display) -> ! {
    eprintln!("rikka: {}: {message}", ENTRY.get());
    std::process::abort()
}
