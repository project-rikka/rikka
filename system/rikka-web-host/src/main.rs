use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::process::ExitCode;

const CHANNEL: &str = "RIKKA_WEB_CHANNEL";

const RELAY_BUFFER: usize = 64 * 1024;

fn main() -> ExitCode {
    let Some(path) = std::env::var_os(CHANNEL) else {
        eprintln!("rikka-web-host: {CHANNEL} is unset; Chrome was not started by Rikka");
        return ExitCode::FAILURE;
    };
    let channel = match UnixStream::connect(&path) {
        Ok(channel) => channel,
        Err(error) => {
            eprintln!(
                "rikka-web-host: failed to connect to {}: {error}",
                path.display()
            );
            return ExitCode::FAILURE;
        }
    };
    let mut outgoing = match channel.try_clone() {
        Ok(outgoing) => outgoing,
        Err(error) => {
            eprintln!("rikka-web-host: failed to share the channel: {error}");
            return ExitCode::FAILURE;
        }
    };
    let upstream = std::thread::Builder::new()
        .name("rikka-web-host-up".into())
        .spawn(move || {
            let copied = io::copy(&mut io::stdin().lock(), &mut outgoing);
            if let Err(error) = outgoing.shutdown(Shutdown::Write) {
                eprintln!("rikka-web-host: failed to close the channel: {error}");
            }
            copied
        });
    if let Err(error) = upstream {
        eprintln!("rikka-web-host: failed to start relaying: {error}");
        return ExitCode::FAILURE;
    }
    match relay(channel) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("rikka-web-host: failed to relay to Chrome: {error}");
            ExitCode::FAILURE
        }
    }
}

fn relay(mut incoming: UnixStream) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    let mut buffer = vec![0; RELAY_BUFFER];
    loop {
        let read = incoming.read(&mut buffer)?;
        if read == 0 {
            return Ok(());
        }
        stdout.write_all(&buffer[..read])?;
        stdout.flush()?;
    }
}
