use std::process::ExitCode;

const USAGE: &str = "usage: rikka-pam-helper <service> <user>";

fn main() -> ExitCode {
    rikka_log::install("warn");
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [service, user] = arguments.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    match rikka_pam::serve(service, user) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("rikka-pam-helper: {error}");
            ExitCode::FAILURE
        }
    }
}
