use std::os::fd::OwnedFd;
use std::time::{Duration, Instant};

use rikka_clipboard::Offer;
use rustix::event::{PollFd, PollFlags, Timespec};
use rustix::io::Errno;
use rustix::pipe::PipeFlags;

const PATIENCE: Duration = Duration::from_secs(3);
const LIMIT: usize = 64 << 20;
const CHUNK: usize = 64 << 10;

pub(crate) struct Pending {
    mime: String,
    fd: OwnedFd,
    data: Vec<u8>,
    complete: bool,
}

pub(crate) fn pipe(mime: &str) -> Option<(Pending, OwnedFd)> {
    let (read, write) = rustix::pipe::pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK)
        .inspect_err(|error| tracing::warn!("failed to create transfer pipe: {error}"))
        .ok()?;
    rustix::io::ioctl_fionbio(&write, false).ok()?;
    Some((
        Pending {
            mime: mime.to_owned(),
            fd: read,
            data: Vec::new(),
            complete: false,
        },
        write,
    ))
}

pub(crate) fn gather(mut pending: Vec<Pending>) -> Offer {
    let deadline = Instant::now() + PATIENCE;
    let mut open: Vec<usize> = (0..pending.len()).collect();
    let mut total = 0usize;
    let mut chunk = vec![0u8; CHUNK];
    while !open.is_empty() {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        let timeout = Timespec {
            tv_sec: left.as_secs() as i64,
            tv_nsec: i64::from(left.subsec_nanos()),
        };
        let mut polled: Vec<PollFd<'_>> = open
            .iter()
            .map(|index| PollFd::new(&pending[*index].fd, PollFlags::IN))
            .collect();
        match rustix::event::poll(&mut polled, Some(&timeout)) {
            Ok(_) => {}
            Err(Errno::INTR) => continue,
            Err(error) => {
                tracing::warn!("failed to poll transfer pipes: {error}");
                break;
            }
        }
        let ready: Vec<usize> = polled
            .iter()
            .zip(&open)
            .filter(|(fd, _)| !fd.revents().is_empty())
            .map(|(_, index)| *index)
            .collect();
        drop(polled);
        for index in ready {
            let entry = &mut pending[index];
            let finished = loop {
                match rustix::io::read(&entry.fd, &mut chunk) {
                    Ok(0) => break Some(true),
                    Ok(read) => {
                        total += read;
                        if total > LIMIT {
                            break Some(false);
                        }
                        entry.data.extend_from_slice(&chunk[..read]);
                    }
                    Err(Errno::AGAIN) => break None,
                    Err(Errno::INTR) => {}
                    Err(_) => break Some(false),
                }
            };
            if let Some(complete) = finished {
                entry.complete = complete;
                open.retain(|known| *known != index);
            }
        }
        if total > LIMIT {
            tracing::warn!("transfer truncated at {LIMIT} bytes");
            break;
        }
    }
    pending
        .into_iter()
        .filter(|entry| entry.complete)
        .fold(Offer::new(), |offer, entry| {
            offer.with(entry.mime, entry.data)
        })
}
