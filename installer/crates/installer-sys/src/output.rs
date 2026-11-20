use std::{
    io::{BufRead, BufReader, Read, Write},
    process::Child,
    sync::mpsc,
    thread,
};

use crate::{
    command::Output,
    error::{Error, Result},
};

pub(crate) fn feed(child: &mut Child, secret: &str) -> Result<()> {
    let mut stdin = child.stdin.take().ok_or_else(|| Error::Io {
        path: "stdin".to_owned(),
        source: std::io::Error::other("standard input was not piped"),
    })?;
    stdin
        .write_all(secret.as_bytes())
        .map_err(|source| Error::Io {
            path: "stdin".to_owned(),
            source,
        })
}

type Line = (bool, String);

pub(crate) fn collect(mut child: Child, on_line: &mut dyn FnMut(&str)) -> Result<Output> {
    let (sender, receiver) = mpsc::channel::<Line>();
    let mut readers = Vec::new();

    if let Some(stdout) = child.stdout.take() {
        readers.push(pump(stdout, false, sender.clone()));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(pump(stderr, true, sender.clone()));
    }
    drop(sender);

    let mut output = Output::default();
    for (is_stderr, line) in receiver {
        on_line(&line);
        let sink = if is_stderr {
            &mut output.stderr
        } else {
            &mut output.stdout
        };
        sink.push_str(&line);
        sink.push('\n');
        keep_tail(sink);
    }
    for reader in readers {
        if let Err(panic) = reader.join() {
            std::panic::resume_unwind(panic);
        }
    }

    let status = child.wait().map_err(|source| Error::Io {
        path: "wait".to_owned(),
        source,
    })?;
    output.status = status.code().unwrap_or(-1);
    Ok(output)
}

fn pump(
    stream: impl Read + Send + 'static,
    is_stderr: bool,
    sender: mpsc::Sender<Line>,
) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name("rikka-installer-output".to_owned())
        .spawn(move || {
            let mut reader = BufReader::new(stream);
            let mut raw = Vec::new();
            while reader.read_until(b'\n', &mut raw).unwrap_or(0) > 0 {
                let text = String::from_utf8_lossy(&raw);
                for line in sanitize(&text) {
                    if sender.send((is_stderr, line)).is_err() {
                        return;
                    }
                }
                raw.clear();
            }
        })
        .expect("thread should spawn")
}

const KEEP: usize = 64 * 1024;

fn keep_tail(sink: &mut String) {
    if sink.len() <= KEEP * 2 {
        return;
    }
    let mut cut = sink.len() - KEEP;
    while !sink.is_char_boundary(cut) {
        cut += 1;
    }
    sink.drain(..cut);
}

fn sanitize(chunk: &str) -> Vec<String> {
    chunk
        .split(['\r', '\n'])
        .map(strip_ansi)
        .filter(|line| !line.trim().is_empty())
        .collect()
}

fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            if !ch.is_control() || ch == '\t' {
                out.push(ch);
            }
            continue;
        }
        match chars.next() {
            Some('[') => {
                for ch in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&ch) {
                        break;
                    }
                }
            }
            Some(']') => {
                for ch in chars.by_ref() {
                    if ch == '\u{7}' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out.trim_end().to_owned()
}
