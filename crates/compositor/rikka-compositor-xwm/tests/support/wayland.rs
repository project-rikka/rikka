use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

const GLOBALS: [(u32, &str, u32); 4] = [
    (1, "wl_compositor", 4),
    (2, "wl_shm", 1),
    (3, "wl_output", 2),
    (4, "xwayland_shell_v1", 1),
];

#[derive(Debug, Default)]
pub(crate) struct Serials {
    recorded: Mutex<Vec<u64>>,
    changed: Condvar,
}

impl Serials {
    fn record(&self, serial: u64) {
        self.recorded.lock().unwrap().push(serial);
        self.changed.notify_all();
    }

    pub(crate) fn wait_for(&self, serial: u64, deadline: Instant) {
        let left = deadline.saturating_duration_since(Instant::now());
        let (recorded, _) = self
            .changed
            .wait_timeout_while(self.recorded.lock().unwrap(), left, |recorded| {
                !recorded.contains(&serial)
            })
            .unwrap();
        assert!(
            recorded.contains(&serial),
            "Xwayland never sent serial {serial}"
        );
    }
}

enum Arg<'a> {
    Uint(u32),
    Int(i32),
    Text(&'a str),
}

pub(crate) fn serve(stream: UnixStream) -> Arc<Serials> {
    let serials = Arc::new(Serials::default());
    let recorded = Arc::clone(&serials);
    std::thread::Builder::new()
        .name("rikka-fake-wayland".into())
        .spawn(move || {
            let mut compositor = Compositor {
                stream,
                objects: HashMap::from([(1, "wl_display".to_owned())]),
                serials: recorded,
                sent: 0,
            };
            compositor.run();
        })
        .expect("the fake Wayland compositor thread starts");
    serials
}

struct Compositor {
    stream: UnixStream,
    objects: HashMap<u32, String>,
    serials: Arc<Serials>,
    sent: u32,
}

impl Compositor {
    fn run(&mut self) {
        loop {
            let mut header = [0u8; 8];
            if self.stream.read_exact(&mut header).is_err() {
                return;
            }
            let object = u32::from_ne_bytes(header[..4].try_into().unwrap());
            let word = u32::from_ne_bytes(header[4..].try_into().unwrap());
            let mut body = vec![0u8; ((word >> 16) as usize).saturating_sub(8)];
            if self.stream.read_exact(&mut body).is_err() {
                return;
            }
            if self.request(object, word & 0xffff, &body).is_err() {
                return;
            }
        }
    }

    fn request(&mut self, object: u32, opcode: u32, body: &[u8]) -> io::Result<()> {
        let word = |at: usize| u32::from_ne_bytes(body[at..at + 4].try_into().unwrap());
        let interface = self.objects.get(&object).cloned().unwrap_or_default();
        match (interface.as_str(), opcode) {
            ("wl_display", 0) => self.done(word(0))?,
            ("wl_display", 1) => {
                let registry = word(0);
                self.objects.insert(registry, "wl_registry".into());
                for (name, interface, version) in GLOBALS {
                    self.send(
                        registry,
                        0,
                        &[Arg::Uint(name), Arg::Text(interface), Arg::Uint(version)],
                    )?;
                }
            }
            ("wl_registry", 0) => {
                let length = word(4) as usize;
                let interface = String::from_utf8_lossy(&body[8..8 + length - 1]).into_owned();
                let after = 8 + length.div_ceil(4) * 4;
                let id = word(after + 4);
                self.bound(id, &interface)?;
                self.objects.insert(id, interface);
            }
            ("wl_compositor", 0) => {
                self.objects.insert(word(0), "wl_surface".into());
            }
            ("wl_surface", 3) => self.done(word(0))?,
            ("xwayland_shell_v1", 1) => {
                self.objects.insert(word(0), "xwayland_surface_v1".into());
            }
            ("xwayland_surface_v1", 0) => {
                let serial = u64::from(word(0)) | (u64::from(word(4)) << 32);
                self.serials.record(serial);
            }
            _ => {}
        }
        Ok(())
    }

    fn bound(&mut self, id: u32, interface: &str) -> io::Result<()> {
        match interface {
            "wl_shm" => {
                self.send(id, 0, &[Arg::Uint(0)])?;
                self.send(id, 0, &[Arg::Uint(1)])
            }
            "wl_output" => {
                self.send(
                    id,
                    0,
                    &[
                        Arg::Int(0),
                        Arg::Int(0),
                        Arg::Int(600),
                        Arg::Int(340),
                        Arg::Int(0),
                        Arg::Text("Rikka"),
                        Arg::Text("Test"),
                        Arg::Int(0),
                    ],
                )?;
                self.send(
                    id,
                    1,
                    &[
                        Arg::Uint(3),
                        Arg::Int(1920),
                        Arg::Int(1080),
                        Arg::Int(60_000),
                    ],
                )?;
                self.send(id, 3, &[Arg::Int(1)])?;
                self.send(id, 2, &[])
            }
            _ => Ok(()),
        }
    }

    fn done(&mut self, callback: u32) -> io::Result<()> {
        self.sent += 1;
        self.send(callback, 0, &[Arg::Uint(self.sent)])?;
        self.send(1, 1, &[Arg::Uint(callback)])
    }

    fn send(&mut self, object: u32, opcode: u32, args: &[Arg<'_>]) -> io::Result<()> {
        let mut body = Vec::new();
        for arg in args {
            match arg {
                Arg::Uint(value) => body.extend_from_slice(&value.to_ne_bytes()),
                Arg::Int(value) => body.extend_from_slice(&value.to_ne_bytes()),
                Arg::Text(text) => {
                    let length = u32::try_from(text.len() + 1).unwrap();
                    body.extend_from_slice(&length.to_ne_bytes());
                    body.extend_from_slice(text.as_bytes());
                    body.push(0);
                    body.resize(body.len().div_ceil(4) * 4, 0);
                }
            }
        }
        let size = u32::try_from(body.len() + 8).unwrap();
        let mut message = Vec::with_capacity(body.len() + 8);
        message.extend_from_slice(&object.to_ne_bytes());
        message.extend_from_slice(&((size << 16) | opcode).to_ne_bytes());
        message.extend_from_slice(&body);
        self.stream.write_all(&message)
    }
}
