mod error;
mod polkit;
mod recorder;
mod run_guard;
mod serve;
mod service;

use std::process::ExitCode;

use rikka_nix_protocol::SERVICE;

fn main() -> ExitCode {
    rikka_log::install("warn");
    if unsafe { libc::geteuid() } != 0 {
        eprintln!("rikka-nixctl must run as root; systemd activates it for {SERVICE}");
        return ExitCode::FAILURE;
    }

    match async_io::block_on(serve::run()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
