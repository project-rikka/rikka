use std::path::PathBuf;
use std::process::ExitCode;

const RUNTIME: &str = "rikka_wayland_wire";
const SHARED: &str = "rikka_wayland_protocols";
const USAGE: &str = "usage: generate <shared|server|client> OUTPUT PROTOCOL.xml...";

fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);
    let (Some(side), Some(output)) = (arguments.next(), arguments.next()) else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    let output = PathBuf::from(output);
    let mut protocols: Vec<PathBuf> = arguments.map(PathBuf::from).collect();
    protocols.sort_by_key(|path| path.to_string_lossy().into_owned());
    let generated = match side.to_str() {
        Some("shared") => rikka_wayland_scanner::shared(&protocols, RUNTIME),
        Some("server") => rikka_wayland_scanner::server(&protocols, RUNTIME, SHARED),
        Some("client") => rikka_wayland_scanner::client(&protocols, RUNTIME, SHARED),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let generated = match generated {
        Ok(generated) => generated,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    match std::fs::write(&output, generated) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("failed to write {}: {error}", output.display());
            ExitCode::FAILURE
        }
    }
}
