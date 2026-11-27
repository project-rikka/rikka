use std::io::{ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use rikka_net::Url;
use rustls::crypto::CryptoProvider;
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};

use crate::error::{Error, Result};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const IO_TIMEOUT: Duration = Duration::from_secs(30);
const HEAD_END: &[u8] = b"\r\n\r\n";
const LINE_END: &[u8] = b"\r\n";

#[derive(Debug)]
pub(crate) struct Answer {
    pub(crate) status: u16,
    pub(crate) body: String,
}

pub(crate) fn post_form(url: &Url, agent: &str, form: &str) -> Result<Answer> {
    let host = url
        .host_str()
        .ok_or_else(|| Error::Endpoint("no host".to_owned()))?;
    let secure = url.scheme() == "https";
    let port = url
        .port_or_known_default()
        .ok_or_else(|| Error::Endpoint("no port".to_owned()))?;
    let mut request = Vec::with_capacity(form.len() + 256);
    write!(
        request,
        "POST {} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: {agent}\r\nAccept: */*\r\n\
         Accept-Encoding: identity\r\nContent-Type: application/x-www-form-urlencoded\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{form}",
        url.path(),
        form.len(),
    )
    .map_err(unreachable)?;

    let stream = connect(host, port)?;
    match secure {
        true => {
            let name = ServerName::try_from(host.to_owned())
                .map_err(|error| Error::Endpoint(error.to_string()))?;
            let connection = ClientConnection::new(tls_config()?, name).map_err(unreachable)?;
            exchange(StreamOwned::new(connection, stream), &request)
        }
        false => exchange(stream, &request),
    }
}

fn connect(host: &str, port: u16) -> Result<TcpStream> {
    let mut failure = None;
    for address in (host, port).to_socket_addrs().map_err(unreachable)? {
        match TcpStream::connect_timeout(&address, CONNECT_TIMEOUT) {
            Ok(stream) => {
                stream
                    .set_read_timeout(Some(IO_TIMEOUT))
                    .map_err(unreachable)?;
                stream
                    .set_write_timeout(Some(IO_TIMEOUT))
                    .map_err(unreachable)?;
                return Ok(stream);
            }
            Err(error) => failure = Some(error),
        }
    }
    Err(failure.map_or_else(
        || Error::Unreachable("the host has no address".to_owned()),
        unreachable,
    ))
}

fn exchange(mut stream: impl Read + Write, request: &[u8]) -> Result<Answer> {
    stream.write_all(request).map_err(unreachable)?;
    stream.flush().map_err(unreachable)?;
    let mut raw = Vec::new();
    let mut chunk = [0; 8192];
    loop {
        let read = match stream.read(&mut chunk) {
            Ok(read) => read,
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => 0,
            Err(error) => return Err(unreachable(error)),
        };
        raw.extend_from_slice(&chunk[..read]);
        let ended = read == 0;
        if let Some(answer) = parse(&raw, ended)? {
            return Ok(answer);
        }
        if ended {
            return Err(Error::Malformed("truncated answer"));
        }
    }
}

fn parse(raw: &[u8], ended: bool) -> Result<Option<Answer>> {
    let Some(head_end) = find(raw, HEAD_END) else {
        return Ok(None);
    };
    let head = std::str::from_utf8(&raw[..head_end]).map_err(|_| Error::Malformed("head"))?;
    let status = head
        .split("\r\n")
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|status| status.parse().ok())
        .ok_or(Error::Malformed("status line"))?;
    let header = |name: &str| {
        head.split("\r\n").skip(1).find_map(|line| {
            let (found, value) = line.split_once(':')?;
            found
                .trim()
                .eq_ignore_ascii_case(name)
                .then(|| value.trim().to_ascii_lowercase())
        })
    };
    let rest = &raw[head_end + HEAD_END.len()..];
    let body = if header("transfer-encoding").is_some_and(|coding| coding.contains("chunked")) {
        dechunk(rest)
    } else {
        match header("content-length").and_then(|length| length.parse::<usize>().ok()) {
            Some(length) => rest.get(..length).map(<[u8]>::to_vec),
            None => ended.then(|| rest.to_vec()),
        }
    };
    Ok(body.map(|body| Answer {
        status,
        body: String::from_utf8_lossy(&body).into_owned(),
    }))
}

fn dechunk(mut rest: &[u8]) -> Option<Vec<u8>> {
    let mut body = Vec::new();
    loop {
        let line_end = find(rest, LINE_END)?;
        let size = std::str::from_utf8(&rest[..line_end]).ok()?;
        let size = usize::from_str_radix(size.split(';').next()?.trim(), 16).ok()?;
        if size == 0 {
            return Some(body);
        }
        let start = line_end + LINE_END.len();
        body.extend_from_slice(rest.get(start..start + size)?);
        rest = rest.get(start + size + LINE_END.len()..)?;
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn tls_config() -> Result<Arc<ClientConfig>> {
    static CONFIG: OnceLock<Arc<ClientConfig>> = OnceLock::new();
    if let Some(built) = CONFIG.get() {
        return Ok(Arc::clone(built));
    }
    let provider = CryptoProvider::get_default()
        .cloned()
        .unwrap_or_else(|| Arc::new(rustls::crypto::aws_lc_rs::default_provider()));
    let mut roots = RootCertStore::empty();
    let loaded = rustls_native_certs::load_native_certs();
    for error in &loaded.errors {
        tracing::debug!(%error, "failed to load system certificate");
    }
    roots.add_parsable_certificates(loaded.certs);
    if roots.is_empty() {
        return Err(Error::Unreachable(
            "there are no certificate authorities to trust".to_owned(),
        ));
    }
    let config = ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS12, &rustls::version::TLS13])
        .map_err(unreachable)?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(Arc::clone(CONFIG.get_or_init(|| Arc::new(config))))
}

fn unreachable(error: impl std::fmt::Display) -> Error {
    Error::Unreachable(error.to_string())
}
