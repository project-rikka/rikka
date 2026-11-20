use std::backtrace::Backtrace;

const DEFAULT_FILTER: &str = "info,rikka_shell=debug,shell_=debug";

pub(crate) fn install() {
    rikka_log::install(DEFAULT_FILTER);
    std::panic::set_hook(Box::new(|panic| {
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("unnamed");
        eprintln!(
            "thread '{name}' {panic}\nstack backtrace:\n{}",
            Backtrace::force_capture()
        );
    }));
}
