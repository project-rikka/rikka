use std::io::{self, BufReader, BufWriter, Read, Write};
use std::os::unix::net::UnixStream;
use std::thread;

use crate::listening::Listening;
use crate::transport::Transport;

fn send(stream: &mut impl Write, text: &str) -> io::Result<()> {
    let body = serde_json::to_vec(text)?;
    stream.write_all(&(body.len() as u32).to_le_bytes())?;
    stream.write_all(&body)
}

fn receive(stream: &mut impl Read, body: &mut Vec<u8>) -> io::Result<String> {
    let mut length = [0; 4];
    stream.read_exact(&mut length)?;
    body.resize(u32::from_le_bytes(length) as usize, 0);
    stream.read_exact(body)?;
    Ok(serde_json::from_slice(body)?)
}

fn serve(stream: UnixStream) -> io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = BufWriter::new(stream);
    let mut body = Vec::new();
    loop {
        let text = match receive(&mut reader, &mut body) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(error) => return Err(error),
        };
        send(&mut writer, &text)?;
        if reader.buffer().is_empty() {
            writer.flush()?;
        }
    }
}

pub struct Socket {
    reader: BufReader<UnixStream>,
    writer: BufWriter<UnixStream>,
    body: Vec<u8>,
}

impl Transport for Socket {
    const NAME: &'static str = "unix-socket";

    type Server = Listening;

    fn serve() -> Listening {
        Listening::start("socket", |stream| {
            thread::Builder::new()
                .name("rikka-bench-echo".to_owned())
                .spawn(move || serve(stream))
                .expect("thread for the connection");
        })
    }

    fn connect(server: &Listening) -> Self {
        let client = UnixStream::connect(server.path()).expect("listener should accept");
        Self {
            reader: BufReader::new(client.try_clone().expect("socket should clone")),
            writer: BufWriter::new(client),
            body: Vec::new(),
        }
    }

    fn echo(&mut self, text: &str) -> String {
        send(&mut self.writer, text).expect("request should be sent");
        self.writer.flush().expect("request should be flushed");
        receive(&mut self.reader, &mut self.body).expect("echo should answer")
    }

    fn echo_many(&mut self, text: &str, count: usize) -> usize {
        for _ in 0..count {
            send(&mut self.writer, text).expect("request should be sent");
        }
        self.writer.flush().expect("requests should be flushed");
        (0..count)
            .map(|_| receive(&mut self.reader, &mut self.body).expect("echo should answer"))
            .count()
    }
}

impl std::fmt::Debug for Socket {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Socket").finish_non_exhaustive()
    }
}
