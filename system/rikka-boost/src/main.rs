mod clocks;
mod error;
mod serve;

use std::process::ExitCode;

use rikka_boost::service::SERVICE;

use crate::clocks::Clocks;

const LOWER: &str = "--lower";

fn main() -> ExitCode {
    rikka_log::install("warn");
    if unsafe { libc::geteuid() } != 0 {
        eprintln!("rikka-boost must run as root; systemd activates it for {SERVICE}");
        return ExitCode::FAILURE;
    }

    let clocks = match Clocks::open() {
        Ok(clocks) => clocks,
        Err(error) => {
            eprintln!("no GPU to boost: {error}");
            return ExitCode::SUCCESS;
        }
    };
    clocks.lower();
    if std::env::args().skip(1).any(|argument| argument == LOWER) {
        return ExitCode::SUCCESS;
    }

    match async_io::block_on(serve::run(clocks)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
