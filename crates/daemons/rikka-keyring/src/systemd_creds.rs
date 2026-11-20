use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};
use zeroize::Zeroizing;

pub(crate) const BINDING_NAME: &str = "systemd-creds";

const SOCKET: &str = "/run/systemd/io.systemd.Credentials";
const CREDENTIAL: &str = "rikka-keyring";
const ENCRYPT: &str = "io.systemd.Credentials.Encrypt";
const DECRYPT: &str = "io.systemd.Credentials.Decrypt";
const USER_SCOPE: &str = "user";
const REPLY_TIMEOUT: Duration = Duration::from_secs(30);
const TERMINATOR: u8 = 0;

pub(crate) fn encrypt(plain: &[u8]) -> Result<Vec<u8>, String> {
    let data = Zeroizing::new(STANDARD.encode(plain));
    let reply = call(
        ENCRYPT,
        json!({
            "name": CREDENTIAL,
            "data": data.as_str(),
            "scope": USER_SCOPE,
            "uid": user(),
        }),
    )?;
    reply
        .get("blob")
        .and_then(Value::as_str)
        .map(|blob| blob.as_bytes().to_vec())
        .ok_or_else(|| "systemd-creds returned no blob".to_owned())
}

pub(crate) fn decrypt(blob: &[u8]) -> Result<Zeroizing<Vec<u8>>, String> {
    let blob = std::str::from_utf8(blob).map_err(|_| "credential blob is not text".to_owned())?;
    let reply = call(
        DECRYPT,
        json!({
            "name": CREDENTIAL,
            "blob": blob,
            "scope": USER_SCOPE,
            "uid": user(),
        }),
    )?;
    let data = reply
        .get("data")
        .and_then(Value::as_str)
        .ok_or_else(|| "systemd-creds returned no data".to_owned())?;
    STANDARD
        .decode(data)
        .map(Zeroizing::new)
        .map_err(|error| format!("systemd-creds returned malformed data: {error}"))
}

fn user() -> u32 {
    rustix::process::getuid().as_raw()
}

fn call(method: &str, parameters: Value) -> Result<Value, String> {
    let failed = |error: std::io::Error| format!("{SOCKET}: {error}");
    let mut stream = UnixStream::connect(SOCKET).map_err(failed)?;
    stream
        .set_read_timeout(Some(REPLY_TIMEOUT))
        .map_err(failed)?;
    let mut request = Zeroizing::new(
        serde_json::to_vec(&json!({ "method": method, "parameters": parameters }))
            .map_err(|error| error.to_string())?,
    );
    request.push(TERMINATOR);
    stream.write_all(&request).map_err(failed)?;
    let mut reply = Zeroizing::new(Vec::new());
    BufReader::new(&stream)
        .read_until(TERMINATOR, &mut reply)
        .map_err(failed)?;
    if reply.pop() != Some(TERMINATOR) {
        return Err(format!("{SOCKET} closed before replying"));
    }
    let mut reply: Value = serde_json::from_slice(&reply).map_err(|error| error.to_string())?;
    if let Some(error) = reply.get("error").and_then(Value::as_str) {
        return Err(format!("{method} failed: {error}"));
    }
    Ok(reply
        .get_mut("parameters")
        .map(Value::take)
        .unwrap_or(Value::Null))
}
