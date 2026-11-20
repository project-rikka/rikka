use rikka_sync::Mutex;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

use rikka_llm::Model;
use rikka_llm::wire::{
    CloudCode, Completions, CompletionsDialect, Messages, Responses, ResponsesDialect, Transport,
    endpoint,
};
use rikka_net::HeaderMap;
use serde_json::Value;

use crate::exchange::{Exchange, Reply};
use crate::frames;

type Answering = Arc<dyn Fn(&Exchange) -> Option<Vec<u8>> + Send + Sync>;

#[derive(Clone)]
pub struct Server {
    url: String,
    exchanges: Arc<Mutex<Vec<Exchange>>>,
}

impl Server {
    pub(crate) fn answering(answer: impl Fn(&Value) -> Vec<u8> + Send + Sync + 'static) -> Server {
        Server::serving(move |exchange: &Exchange| exchange.body().map(&answer))
    }

    #[must_use]
    pub fn routed(answer: impl Fn(&Exchange) -> Reply + Send + Sync + 'static) -> Server {
        Server::serving(move |exchange: &Exchange| Some(answer(exchange).into_bytes()))
    }

    fn serving(answer: impl Fn(&Exchange) -> Option<Vec<u8>> + Send + Sync + 'static) -> Server {
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("test server should bind a free port");
        let url = format!(
            "http://{}",
            listener
                .local_addr()
                .expect("listener should have a local address")
        );
        let exchanges = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&exchanges);
        let answer: Answering = Arc::new(answer);

        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else {
                    return;
                };
                let recorded = Arc::clone(&recorded);
                let answer = Arc::clone(&answer);
                std::thread::spawn(move || serve(stream, &recorded, &answer));
            }
        });

        Server { url, exchanges }
    }

    #[must_use]
    pub fn replying(reply: Vec<u8>) -> Server {
        Server::answering(move |_| reply.clone())
    }

    #[must_use]
    pub fn streaming(frames: &[&str]) -> Server {
        Server::replying(frames::events(
            frames.iter().map(|frame| (*frame).to_owned()),
        ))
    }

    #[must_use]
    pub fn failing(status: u16, body: &Value) -> Server {
        Server::replying(frames::json(status, body))
    }

    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    #[must_use]
    pub fn requests(&self) -> Vec<Value> {
        self.exchanges
            .lock()
            .iter()
            .filter_map(|exchange| exchange.body().cloned())
            .collect()
    }

    #[must_use]
    pub fn exchanges(&self) -> Vec<Exchange> {
        self.exchanges.lock().clone()
    }

    #[must_use]
    pub fn last(&self) -> Value {
        self.requests()
            .pop()
            .expect("server should have received a request")
    }

    #[must_use]
    pub fn messages(&self, model: &str) -> Model {
        Messages::new(self.transport(), model).build()
    }

    #[must_use]
    pub fn responses(&self, model: &str, dialect: ResponsesDialect) -> Model {
        Responses::new(self.transport(), model, dialect).build()
    }

    #[must_use]
    pub fn completions(&self, model: &str, dialect: CompletionsDialect) -> Model {
        Completions::new(self.transport(), model, dialect).build()
    }

    #[must_use]
    pub fn cloud_code(&self, model: &str) -> Model {
        CloudCode::new(self.transport(), model)
            .project("projects/42")
            .session("thread-1")
            .build()
    }

    fn transport(&self) -> Transport {
        Transport::new(
            endpoint(&self.url).expect("test server URL should parse"),
            HeaderMap::new(),
        )
        .expect("test transport should build")
    }
}

impl std::fmt::Debug for Server {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Server")
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

fn serve(mut stream: TcpStream, recorded: &Mutex<Vec<Exchange>>, answer: &Answering) {
    let Some(exchange) = read_exchange(&mut stream) else {
        return;
    };
    recorded.lock().push(exchange.clone());
    let Some(reply) = answer(&exchange) else {
        return;
    };
    let _ = stream.write_all(&reply);
    let _ = stream.flush();
}

fn read_exchange(stream: &mut TcpStream) -> Option<Exchange> {
    let mut reader = BufReader::new(stream);
    let mut request = String::new();
    reader.read_line(&mut request).ok()?;
    let mut parts = request.split_whitespace();
    let method = parts.next()?.to_owned();
    let path = parts.next()?.to_owned();

    let mut headers = Vec::new();
    let mut length = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            let value = value.trim();
            if name.eq_ignore_ascii_case("content-length") {
                length = value.parse().ok()?;
            }
            headers.push((name.to_ascii_lowercase(), value.to_owned()));
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(Exchange::new(
        method,
        path,
        headers,
        serde_json::from_slice(&body).ok(),
    ))
}
