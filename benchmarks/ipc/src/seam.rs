use std::path::PathBuf;

use rikka_seam::net::{SocketClient, SocketListener};
use rikka_seam::{Call, Manifest, Method, MethodInfo, Request, SeamClient, Service, Version};
use rikka_test_seam::Host;

use crate::transport::Transport;

const CAPABILITY: &str = "bench.echo";

struct Echo;

impl Service for Echo {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, Version::new(1, 0)).method(MethodInfo::new("echo"))
    }

    fn call(&mut self, _: &mut (), method: Method, call: Call) {
        match &*method {
            "echo" => match call.decode::<String>() {
                Ok(text) => call.encode(&text),
                Err(error) => call.fail(error),
            },
            _ => call.unknown_method(),
        }
    }
}

pub struct Seam {
    client: SeamClient,
    _socket: SocketClient,
}

pub struct SeamServer {
    _listener: SocketListener,
    _host: Host,
    directory: tempfile::TempDir,
}

impl SeamServer {
    fn path(&self) -> PathBuf {
        self.directory.path().join("bench.sock")
    }
}

impl Transport for Seam {
    const NAME: &'static str = "rikka-seam";

    type Server = SeamServer;

    fn serve() -> SeamServer {
        let directory = tempfile::tempdir().expect("temporary directory");
        let host = Host::start(|seam| {
            seam.register(Echo).expect("echo service should register");
        });
        let listener =
            SocketListener::serve(directory.path().join("bench.sock"), host.client().clone())
                .expect("socket should bind");
        SeamServer {
            _listener: listener,
            _host: host,
            directory,
        }
    }

    fn connect(server: &SeamServer) -> Self {
        let socket = SocketClient::connect(server.path()).expect("listener should accept");
        Self {
            client: socket.client(),
            _socket: socket,
        }
    }

    fn echo(&mut self, text: &str) -> String {
        self.client
            .call(Request::new(CAPABILITY, "echo").encode(&text))
            .blocking_wait()
            .expect("echo should answer")
            .decode()
            .expect("echo should be a string")
    }

    fn echo_many(&mut self, text: &str, count: usize) -> usize {
        let replies: Vec<_> = (0..count)
            .map(|_| {
                self.client
                    .call(Request::new(CAPABILITY, "echo").encode(&text))
            })
            .collect();
        let mut response_count = 0;
        for reply in replies {
            reply.blocking_wait().expect("echo should answer");
            response_count += 1;
        }
        response_count
    }
}

impl std::fmt::Debug for SeamServer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("SeamServer").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Seam {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Seam").finish_non_exhaustive()
    }
}
