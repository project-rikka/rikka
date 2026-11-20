use std::borrow::Cow;
use std::collections::VecDeque;
use std::io::{self, ErrorKind, Read, Write};
use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Instant;

use alacritty_terminal::event::{Event, EventListener, OnResize, WindowSize};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::Term;
use alacritty_terminal::tty::Pty;
use alacritty_terminal::vte::ansi::Processor;
use rustix::event::{EventfdFlags, PollFd, PollFlags, Timespec, eventfd, poll};
use rustix::io::Errno;
use rustix::process::{Pid, PidfdFlags, pidfd_open};

use crate::clipboard::{ClipboardAccess, Osc52, Requests, Scanner};

const CHUNK: usize = 64 << 10;

const LOCKED_READ: usize = u16::MAX as usize;

const THREAD: &str = "rikka-terminal-pty";

enum Message {
    Input(Cow<'static, [u8]>),
    Resize(WindowSize),
    Shutdown,
}

#[derive(Clone)]
pub(crate) struct PtyWriter {
    messages: Sender<Message>,
    wake: Arc<OwnedFd>,
}

impl PtyWriter {
    pub(crate) fn write(&self, bytes: impl Into<Cow<'static, [u8]>>) {
        let bytes = bytes.into();
        if !bytes.is_empty() {
            self.send(Message::Input(bytes));
        }
    }

    pub(crate) fn resize(&self, size: WindowSize) {
        self.send(Message::Resize(size));
    }

    pub(crate) fn shut_down(&self) {
        self.send(Message::Shutdown);
    }

    fn send(&self, message: Message) {
        if self.messages.send(message).is_err() {
            return;
        }
        match rustix::io::write(&self.wake, &1_u64.to_ne_bytes()) {
            Ok(_) | Err(Errno::AGAIN) => {}
            Err(error) => tracing::warn!("failed to wake pty thread: {error}"),
        }
    }
}

pub(crate) fn start<L: EventListener + Send + 'static>(
    pty: Pty,
    term: Arc<FairMutex<Term<L>>>,
    listener: L,
    access: ClipboardAccess,
) -> io::Result<PtyWriter> {
    let wake = Arc::new(eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)?);
    let child = i32::try_from(pty.child().id())
        .ok()
        .and_then(Pid::from_raw)
        .ok_or(ErrorKind::InvalidInput)?;
    let child = pidfd_open(child, PidfdFlags::empty())?;
    let (messages, received) = mpsc::channel();
    let writer = PtyWriter {
        messages,
        wake: Arc::clone(&wake),
    };
    let replying = writer.clone();
    let reader = Reader {
        pty,
        child,
        wake,
        received,
        term,
        listener,
        parser: Processor::new(),
        scanner: Scanner::new(access.limit_bytes()),
        requests: Requests::new(access, Arc::new(move |bytes| replying.write(bytes))),
        incoming: vec![0; CHUNK],
        output: Vec::with_capacity(CHUNK),
        sequences: Vec::new(),
        pending: VecDeque::new(),
        written: 0,
        hung_up: false,
    };
    std::thread::Builder::new()
        .name(THREAD.to_owned())
        .spawn(move || reader.run())?;
    Ok(writer)
}

struct Reader<L: EventListener> {
    pty: Pty,
    child: OwnedFd,
    wake: Arc<OwnedFd>,
    received: Receiver<Message>,
    term: Arc<FairMutex<Term<L>>>,
    listener: L,
    parser: Processor,
    scanner: Scanner,
    requests: Requests,
    incoming: Vec<u8>,
    output: Vec<u8>,
    sequences: Vec<Osc52>,
    pending: VecDeque<Cow<'static, [u8]>>,
    written: usize,
    hung_up: bool,
}

#[derive(Clone, Copy)]
struct Ready {
    woken: bool,
    exited: bool,
    readable: bool,
    writable: bool,
}

impl<L: EventListener> Reader<L> {
    fn run(mut self) {
        loop {
            let ready = match self.wait() {
                Ok(Some(ready)) => ready,
                Ok(None) => {
                    self.parser.stop_sync(&mut *self.term.lock());
                    self.listener.send_event(Event::Wakeup);
                    continue;
                }
                Err(error) => {
                    tracing::warn!("failed to poll pty: {error}");
                    break;
                }
            };
            if ready.woken && !self.take_messages() {
                break;
            }
            if ready.exited {
                self.read();
                self.term.lock().exit();
                self.listener.send_event(Event::Wakeup);
                break;
            }
            if ready.readable {
                self.read();
            }
            if ready.writable
                && let Err(error) = self.write()
            {
                tracing::warn!("failed to write to pty: {error}");
                break;
            }
        }
    }

    fn wait(&self) -> io::Result<Option<Ready>> {
        let deadline = self.parser.sync_timeout().sync_timeout();
        let mut interest = PollFlags::IN;
        if !self.pending.is_empty() {
            interest |= PollFlags::OUT;
        }
        let mut fds = vec![
            PollFd::new(&*self.wake, PollFlags::IN),
            PollFd::new(&self.child, PollFlags::IN),
        ];
        if !self.hung_up {
            fds.push(PollFd::new(self.pty.file(), interest));
        }
        loop {
            let timeout = deadline.map(|deadline| {
                let left = deadline.saturating_duration_since(Instant::now());
                Timespec::try_from(left).unwrap_or(Timespec {
                    tv_sec: 0,
                    tv_nsec: 0,
                })
            });
            match poll(&mut fds, timeout.as_ref()) {
                Ok(0) => return Ok(None),
                Ok(_) => break,
                Err(Errno::INTR) => {}
                Err(error) => return Err(error.into()),
            }
        }
        let master = fds.get(2).map_or(PollFlags::empty(), PollFd::revents);
        Ok(Some(Ready {
            woken: !fds[0].revents().is_empty(),
            exited: !fds[1].revents().is_empty(),
            readable: master.intersects(PollFlags::IN | PollFlags::HUP | PollFlags::ERR),
            writable: master.contains(PollFlags::OUT),
        }))
    }

    fn take_messages(&mut self) -> bool {
        let mut count = [0_u8; 8];
        let _ = rustix::io::read(&*self.wake, &mut count);
        while let Ok(message) = self.received.try_recv() {
            match message {
                Message::Input(bytes) => self.pending.push_back(bytes),
                Message::Resize(size) => self.pty.on_resize(size),
                Message::Shutdown => return false,
            }
        }
        true
    }

    fn read(&mut self) {
        let mut processed = 0;
        while processed < LOCKED_READ {
            let mut master = self.pty.file();
            let read = match master.read(&mut self.incoming) {
                Ok(0) => break,
                Ok(read) => read,
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(_) => {
                    self.hung_up = true;
                    break;
                }
            };
            self.output.clear();
            self.scanner.feed(
                &self.incoming[..read],
                &mut self.output,
                &mut self.sequences,
            );
            for sequence in self.sequences.drain(..) {
                self.requests.handle(sequence);
            }
            self.parser.advance(&mut *self.term.lock(), &self.output);
            processed += read;
        }
        if processed > 0 && self.parser.sync_bytes_count() < processed {
            self.listener.send_event(Event::Wakeup);
        }
    }

    fn write(&mut self) -> io::Result<()> {
        while let Some(bytes) = self.pending.front() {
            let mut master = self.pty.file();
            match master.write(&bytes[self.written..]) {
                Ok(0) => break,
                Ok(written) => {
                    self.written += written;
                    if self.written == bytes.len() {
                        self.pending.pop_front();
                        self.written = 0;
                    }
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}
