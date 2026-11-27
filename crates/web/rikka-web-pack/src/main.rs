use std::path::PathBuf;
use std::process::ExitCode;

use rikka_web_crx::Package;

fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);
    let Some(out) = arguments.next().map(PathBuf::from) else {
        eprintln!("usage: rikka-web-pack <out.zip> <path-in-package>=<file>...");
        return ExitCode::FAILURE;
    };
    let mut package = Package::new();
    for argument in arguments {
        let Some((inside, source)) = argument.to_str().and_then(|text| text.split_once('=')) else {
            eprintln!("rikka-web-pack: expected <path-in-package>=<file>, got {argument:?}");
            return ExitCode::FAILURE;
        };
        match std::fs::read(source) {
            Ok(contents) => package = package.file(inside, contents),
            Err(error) => {
                eprintln!("rikka-web-pack: failed to read {source}: {error}");
                return ExitCode::FAILURE;
            }
        }
    }
    let written = package
        .version()
        .map_err(|error| error.to_string())
        .and_then(|_| package.to_zip().map_err(|error| error.to_string()))
        .and_then(|zip| std::fs::write(&out, zip).map_err(|error| error.to_string()));
    match written {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("rikka-web-pack: failed to write {}: {error}", out.display());
            ExitCode::FAILURE
        }
    }
}
