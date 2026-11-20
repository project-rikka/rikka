use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Mutex, mpsc};

use browser_translate::{
    Adapter, Call, Dialect, Http, Limits, Node, Result, Service, Translated, Translating, Unit,
};
use rikka_languages::Language;

pub(crate) type Respond = Box<dyn Fn(&[&str]) -> Result<Vec<Translated>> + Send + Sync>;

pub(crate) struct Fake {
    respond: Respond,
    sent: Mutex<Vec<(Vec<String>, &'static str)>>,
}

impl Fake {
    pub(crate) fn new(
        respond: impl Fn(&[&str]) -> Result<Vec<Translated>> + Send + Sync + 'static,
    ) -> Fake {
        Fake {
            respond: Box::new(respond),
            sent: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn sent(&self) -> Vec<(Vec<String>, &'static str)> {
        std::mem::take(&mut *self.sent.lock().expect("request log lock"))
    }

    pub(crate) fn sizes(&self) -> Vec<usize> {
        let mut sizes: Vec<usize> = self
            .sent()
            .iter()
            .map(|(strings, _)| strings.len())
            .collect();
        sizes.sort_unstable();
        sizes
    }
}

static SERVICE: Service = Service {
    id: "fake",
    label: "Fake",
    dialect: Dialect::new("i").keep("translate=\"no\""),
    targets: &["en", "fr", "ja", "zh-Hans", "zh-Hant"],
    aliases: &[("zh-Hans", "zh-CN"), ("zh-Hant", "zh-TW")],
    limits: Limits::new(4, 60),
};

impl Adapter for Fake {
    fn service(&self) -> &'static Service {
        &SERVICE
    }

    fn translate<'a>(&'a self, _: &'a Http, call: Call<'a>) -> Translating<'a> {
        self.sent.lock().expect("request log lock").push((
            call.strings
                .iter()
                .map(|string| (*string).to_owned())
                .collect(),
            call.to,
        ));
        Box::pin(std::future::ready((self.respond)(call.strings)))
    }
}

pub(crate) fn translated(text: String, detected: &str) -> Translated {
    Translated {
        text,
        detected: Some(detected.to_owned()),
    }
}

pub(crate) fn echo_upper(strings: &[&str]) -> Result<Vec<Translated>> {
    Ok(strings
        .iter()
        .map(|string| translated(string.replace("Hello", "こんにちは"), "en"))
        .collect())
}

pub(crate) fn text(text: &str) -> Unit {
    Unit::new(vec![Node::Text(text.to_owned())]).expect("a valid unit")
}

pub(crate) fn japanese() -> Language {
    Language::lookup("ja").expect("Japanese")
}

#[derive(Clone, Debug)]
pub(crate) struct Recorded {
    pub(crate) method: String,
    pub(crate) target: String,
    headers: Vec<(String, String)>,
    pub(crate) body: String,
}

impl Recorded {
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(recorded, _)| recorded.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

pub(crate) struct Server {
    base: String,
    recorded: mpsc::Receiver<Recorded>,
}

impl Server {
    pub(crate) fn start(answer: impl Fn(&Recorded) -> String + Send + 'static) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local port");
        let base = format!("http://{}", listener.local_addr().expect("local address"));
        let (sender, recorded) = mpsc::channel();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else {
                    return;
                };
                let Some(request) = record(&stream) else {
                    continue;
                };
                let reply = answer(&request);
                let _ = sender.send(request);
                let _ = stream.write_all(reply.as_bytes());
            }
        });
        Server { base, recorded }
    }

    pub(crate) fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    pub(crate) fn recorded(&self) -> Vec<Recorded> {
        self.recorded.try_iter().collect()
    }
}

pub(crate) fn reply(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn record(stream: &std::net::TcpStream) -> Option<Recorded> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_owned();
    let target = parts.next()?.to_owned();
    let mut headers = Vec::new();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap_or(0) == 0 || header == "\r\n" {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
        }
    }
    let length = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .and_then(|(_, length)| length.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(Recorded {
        method,
        target,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}
