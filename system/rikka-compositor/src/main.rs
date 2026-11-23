use std::process::ExitCode;

use rikka_compositor::{Extent, Launch, Logging, Result};

struct Options {
    desktop: bool,
    headless: Option<Extent>,
    socket: Option<String>,
}

enum Screens {
    Kms,
    Nested,
    Headless(Extent),
}

struct UnknownOption(String);

const USAGE: &str = "\
Usage: rikka-compositor [OPTIONS]

The Rikka Wayland compositor.

Options:
  --no-shell        Do not start the desktop shell (rikka-shell); start it later
                    with `rikka desktop start`
  --headless[=WxH]  Run without a display (default 1280x800)
  --socket <NAME>   Wayland socket name
  -h, --help        Print this help

Without --headless the backend follows the environment: nested in an existing
Wayland session, or DRM/KMS on a TTY.

A TTY session starts locked and shows nothing until unlocked (disable with
rikka.lock/at-login; never with --no-shell). It follows loginctl
lock-session and unlock-session.

A nested session runs the full desktop in a window on its own D-Bus session
bus. It reads the user's settings, but changes stay in memory until it exits.
While the window has focus, every key, including Super, goes to the nested
session.

A headless session renders offscreen on its own D-Bus session bus; its output
is reachable only through ext-image-copy-capture-v1. It shows only the
wallpaper, has no workspaces and keeps every window floating.
";

const HEADLESS: Extent = Extent {
    width: 1280,
    height: 800,
};

fn main() -> ExitCode {
    rikka_heap::return_large_glibc_blocks();
    let options = match parse() {
        Ok(Some(options)) => options,
        Ok(None) => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(UnknownOption(argument)) => {
            eprint!("{USAGE}");
            eprintln!("unknown option: {argument}");
            return ExitCode::FAILURE;
        }
    };

    let screens = match options.headless {
        Some(extent) => Screens::Headless(extent),
        None if nested() => Screens::Nested,
        None => Screens::Kms,
    };
    let launch = match screens {
        Screens::Headless(extent) => Launch::headless(extent).private_bus(),
        Screens::Nested => Launch::nested(),
        Screens::Kms => Launch::kms().locked(options.desktop),
    };
    let launch = launch.desktop(options.desktop);
    let launch = match options.socket {
        Some(name) => launch.socket(name),
        None => launch,
    };
    let logging = rikka_compositor::install_logging(launch.journal());
    match screens {
        Screens::Headless(_) => tracing::info!("no host display; running headless"),
        Screens::Nested => tracing::info!("host display found; running nested"),
        Screens::Kms => {}
    }

    unsafe { rikka_gpu_drivers::narrow_system() };

    match run(launch, &logging) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run(launch: Launch, logging: &Logging) -> Result<()> {
    let mut compositor = launch.start()?;
    if let Some(path) = logging.name_after(compositor.socket_name()) {
        tracing::info!("logging to {}", path.display());
    }
    compositor.run()
}

fn nested() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("WAYLAND_SOCKET").is_some()
}

fn parse() -> Result<Option<Options>, UnknownOption> {
    let mut options = Options {
        desktop: true,
        headless: None,
        socket: None,
    };
    let mut arguments = std::env::args().skip(1);

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--no-shell" => options.desktop = false,
            "--headless" => options.headless = Some(HEADLESS),
            "--socket" => match arguments.next() {
                Some(name) => options.socket = Some(name),
                None => return Err(UnknownOption("--socket <NAME>".to_owned())),
            },
            "-h" | "--help" => return Ok(None),
            _ => match named(&argument) {
                Some(("--headless", size)) => options.headless = Some(extent(size)?),
                Some(("--socket", name)) => options.socket = Some(name.to_owned()),
                _ => return Err(UnknownOption(argument)),
            },
        }
    }
    Ok(Some(options))
}

fn named(argument: &str) -> Option<(&str, &str)> {
    let (name, value) = argument.split_once('=')?;
    name.starts_with("--").then_some((name, value))
}

fn extent(size: &str) -> Result<Extent, UnknownOption> {
    let refuse = || UnknownOption(format!("--headless={size}"));
    let (width, height) = size.split_once(['x', 'X']).ok_or_else(refuse)?;
    Ok(Extent {
        width: width.trim().parse().map_err(|_| refuse())?,
        height: height.trim().parse().map_err(|_| refuse())?,
    })
}
