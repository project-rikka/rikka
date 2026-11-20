use std::io::{BufRead, BufReader, Read};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;

use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stream {
    Stdout,
    Stderr,
}

pub(crate) fn run(
    mut command: Command,
    on_line: &mut dyn FnMut(Stream, &str),
) -> Result<ExitStatus> {
    let program = command.get_program().to_string_lossy().into_owned();
    let spawn_error = |source| Error::Spawn {
        program: program.clone(),
        source,
    };
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(spawn_error)?;

    let (sender, receiver) = std::sync::mpsc::channel();
    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");
    let readers = [
        forward_lines(stdout, Stream::Stdout, sender.clone()).map_err(spawn_error)?,
        forward_lines(stderr, Stream::Stderr, sender).map_err(spawn_error)?,
    ];

    for (stream, line) in receiver {
        on_line(stream, &line);
    }
    for reader in readers {
        reader.join().expect("output reader does not panic");
    }

    child
        .wait()
        .map_err(|source| Error::Wait { program, source })
}

fn forward_lines(
    source: impl Read + Send + 'static,
    stream: Stream,
    sender: Sender<(Stream, String)>,
) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("rikka-nixctl-output".to_owned())
        .spawn(move || {
            let mut reader = BufReader::new(source);
            let mut bytes = Vec::new();
            loop {
                bytes.clear();
                match reader.read_until(b'\n', &mut bytes) {
                    Ok(0) => return,
                    Ok(_) => {
                        if bytes.last() == Some(&b'\n') {
                            bytes.pop();
                        }
                        let line = String::from_utf8_lossy(&bytes).into_owned();
                        if sender.send((stream, line)).is_err() {
                            return;
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(error) => {
                        let line = format!("rikka-nixctl: failed to read output: {error}");
                        let _ = sender.send((Stream::Stderr, line));
                        return;
                    }
                }
            }
        })
}
