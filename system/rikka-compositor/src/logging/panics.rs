use rikka_sync::Mutex;
use std::backtrace::Backtrace;
use std::sync::Arc;

use super::Journaled;
use crate::crash::Report;

const PROGRAM: &str = env!("CARGO_PKG_NAME");

pub(super) fn report(journaled: Arc<Mutex<Journaled>>) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        tracing::error!("{panic}");
        let thread = std::thread::current();
        let mut report = Report::new(PROGRAM, std::process::id())
            .field("thread", thread.name().unwrap_or("unnamed"))
            .section("panic", panic)
            .section("backtrace", Backtrace::force_capture());
        if let Some(held) = journaled.try_lock() {
            if let Some(path) = &held.path {
                report = report.field("log", path.display());
            }
            report = report.section("log", held.tail.text());
        }
        match report.write() {
            Ok(path) => tracing::error!("crash report written to {}", path.display()),
            Err(error) => tracing::error!("failed to write crash report: {error}"),
        }
        previous(panic);
    }));
}
