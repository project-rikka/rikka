mod backend;
mod error;
mod picker;
mod protocol;

use std::process::ExitCode;

pub(crate) const PICKER: &str = "--picker";

rikka_app::mimalloc!();

fn main() -> ExitCode {
    rikka_log::install("warn");
    let mut arguments = std::env::args_os().skip(1);
    let (first, rest) = (arguments.next(), arguments.next());
    match (first, rest) {
        (None, _) => backend::run(),
        (Some(argument), None) if argument == PICKER => picker::run(),
        (Some(argument), rest) => {
            eprintln!(
                "rikka-portal-cast: unexpected argument {}; usage: rikka-portal-cast [{PICKER}]",
                rest.unwrap_or(argument).to_string_lossy()
            );
            ExitCode::from(2)
        }
    }
}
