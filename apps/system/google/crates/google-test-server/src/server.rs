use std::io::BufReader;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use rikka_sync::Mutex;

use crate::exchange::{Request, Response};

type Handler = dyn Fn(&Request) -> Response + Send + Sync;

pub struct Server {
    base: String,
    asked: Arc<Mutex<Vec<Request>>>,
    closing: Arc<AtomicBool>,
    address: std::net::SocketAddr,
}

impl Server {
    pub fn start(handler: impl Fn(&Request) -> Response + Send + Sync + 'static) -> Server {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("a free port");
        let address = listener.local_addr().expect("a bound address");
        let asked = Arc::new(Mutex::new(Vec::new()));
        let closing = Arc::new(AtomicBool::new(false));
        let handler: Arc<Handler> = Arc::new(handler);
        let (recorded, closed) = (Arc::clone(&asked), Arc::clone(&closing));
        thread::Builder::new()
            .name("google-test-server".to_owned())
            .spawn(move || {
                for stream in listener.incoming() {
                    if closed.load(Ordering::Acquire) {
                        return;
                    }
                    let Ok(stream) = stream else {
                        continue;
                    };
                    let (handler, recorded) = (Arc::clone(&handler), Arc::clone(&recorded));
                    thread::spawn(move || converse(stream, &*handler, &recorded));
                }
            })
            .expect("a server thread");
        Server {
            base: format!("http://{address}"),
            asked,
            closing,
            address,
        }
    }

    #[must_use]
    pub fn base(&self) -> &str {
        &self.base
    }

    #[must_use]
    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    #[must_use]
    pub fn requests(&self) -> Vec<Request> {
        self.asked.lock().clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.closing.store(true, Ordering::Release);
        drop(TcpStream::connect(self.address));
    }
}

impl std::fmt::Debug for Server {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Server")
            .field("base", &self.base)
            .finish_non_exhaustive()
    }
}

fn converse(stream: TcpStream, handler: &Handler, recorded: &Mutex<Vec<Request>>) {
    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(stream);
    while let Some(request) = Request::read(&mut reader) {
        let response = handler(&request);
        recorded.lock().push(request);
        if response.write(&mut writer).is_err() {
            return;
        }
    }
}
