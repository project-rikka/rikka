use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::Duration;

use futures_util::StreamExt;
use rikka_web::{Runtime, RuntimeEvent};

pub(crate) const PATIENCE: Duration = Duration::from_secs(30);

const STARTUP: Duration = Duration::from_secs(45);

pub(crate) fn served(body: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local port");
    let address = listener.local_addr().expect("local address");
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok_and(|read| read > 2) {
                line.clear();
            }
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(reply.as_bytes());
        }
    });
    format!("http://{address}/")
}

pub(crate) async fn ready(engine: &Runtime) {
    let mut started = engine.events().expect("events not yet taken");
    tokio::time::timeout(STARTUP, async {
        while let Some(event) = started.next().await {
            match event {
                RuntimeEvent::Ready { .. } => return,
                RuntimeEvent::Failed(failure) => panic!("the extension failed: {failure:?}"),
                RuntimeEvent::Exited(exit) => panic!("Chrome exited: {exit:?}"),
                _ => {}
            }
        }
    })
    .await
    .expect("Chrome started in time");
}

pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("create tokio runtime")
        .block_on(future)
}
