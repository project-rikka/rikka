use std::io::Read;
use std::sync::Arc;

use flate2::read::GzDecoder;
use google_sync::proto::ClientToServerMessage;
use google_test_server::{Request, Response, Server};
use prost::Message;
use rikka_sync::Mutex;

use crate::model::{ACCOUNT, FakeServer};

const TOKEN: &str = "ya29.test";

#[derive(Debug)]
pub struct Google {
    pub model: Arc<Mutex<FakeServer>>,
    server: Server,
}

impl Google {
    pub fn start(model: FakeServer) -> Google {
        let model = Arc::new(Mutex::new(model));
        let shared = Arc::clone(&model);
        let server = Server::start(move |request| answer(request, &shared));
        Google { model, server }
    }

    #[must_use]
    pub fn auth(&self) -> String {
        self.server.url("/auth")
    }

    #[must_use]
    pub fn sync(&self) -> String {
        self.server.url("/chrome-sync")
    }

    #[must_use]
    pub fn requests(&self) -> Vec<Request> {
        self.server.requests()
    }
}

fn answer(request: &Request, model: &Mutex<FakeServer>) -> Response {
    match request.path() {
        "/auth" => match request.form().get("service").map(String::as_str) {
            Some("ac2dm") => Response::ok(format!("Token=aas_et/test\nEmail={ACCOUNT}\n")),
            _ => Response::ok(format!("Auth={TOKEN}\nExpiry=99999999999\n")),
        },
        "/chrome-sync/command/" => command(request, model),
        _ => Response::status(404),
    }
}

fn command(request: &Request, model: &Mutex<FakeServer>) -> Response {
    if request.header("authorization") != Some(&format!("Bearer {TOKEN}")) {
        return Response::status(401);
    }
    if request.header("content-encoding") != Some("gzip") {
        return Response::status(400);
    }
    let mut plain = Vec::new();
    if GzDecoder::new(request.body.as_slice())
        .read_to_end(&mut plain)
        .is_err()
    {
        return Response::status(400);
    }
    match ClientToServerMessage::decode(plain.as_slice()) {
        Ok(message) => Response::ok(model.lock().handle(&message).encode_to_vec()),
        Err(_) => Response::status(400),
    }
}
