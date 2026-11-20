use std::os::unix::net::UnixStream;
use std::sync::mpsc;

use zbus::Guid;
use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;

use crate::listening::Listening;
use crate::transport::Transport;

const PATH: &str = "/org/rikka/Bench";
const INTERFACE: &str = "org.rikka.Bench";

struct Echo;

#[zbus::interface(name = "org.rikka.Bench")]
impl Echo {
    fn echo(&self, text: String) -> String {
        text
    }
}

pub struct DBus {
    client: Connection,
    _server: Connection,
}

pub struct DBusServer {
    listening: Listening,
    accepted: mpsc::Receiver<Connection>,
}

impl Transport for DBus {
    const NAME: &'static str = "zbus";

    type Server = DBusServer;

    fn serve() -> DBusServer {
        let (handshaken, accepted) = mpsc::channel();
        let listening = Listening::start("dbus", move |stream| {
            let connection = Builder::async_io_unix_stream(stream)
                .server(Guid::generate())
                .and_then(|builder| builder.p2p().serve_at(PATH, Echo))
                .and_then(Builder::build)
                .expect("D-Bus server handshake should succeed");
            let _ = handshaken.send(connection);
        });
        DBusServer {
            listening,
            accepted,
        }
    }

    fn connect(server: &DBusServer) -> Self {
        let stream = UnixStream::connect(server.listening.path()).expect("listener should accept");
        let client = Builder::async_io_unix_stream(stream)
            .p2p()
            .build()
            .expect("D-Bus client handshake should succeed");
        let accepted = server
            .accepted
            .recv()
            .expect("server handshake should succeed");
        Self {
            client,
            _server: accepted,
        }
    }

    fn echo(&mut self, text: &str) -> String {
        self.client
            .call_method(None::<&str>, PATH, Some(INTERFACE), "Echo", &(text,))
            .expect("echo should answer")
            .body()
            .deserialize::<String>()
            .expect("echo should be a string")
    }

    fn echo_many(&mut self, text: &str, count: usize) -> usize {
        let connection = self.client.inner().clone();
        async_io::block_on(async move {
            let calls = (0..count).map(|_| {
                let connection = connection.clone();
                async move {
                    connection
                        .call_method(None::<&str>, PATH, Some(INTERFACE), "Echo", &(text,))
                        .await
                        .expect("echo should answer")
                }
            });
            futures_util::future::join_all(calls).await.len()
        })
    }
}

impl std::fmt::Debug for DBusServer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("DBusServer").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for DBus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("DBus").finish_non_exhaustive()
    }
}
