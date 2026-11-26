mod error;
mod header;
mod keys;
mod passphrase;
mod stream;

use std::io::{self, BufReader, Read, Write};

use zeroize::Zeroizing;

pub use error::{Error, Result};
pub use stream::{Reader, Writer};

use crate::header::Header;
use crate::keys::{FILE_KEY_BYTES, PAYLOAD_NONCE_BYTES};

pub const DEFAULT_WORK_FACTOR: u8 = 18;
pub const MAX_WORK_FACTOR: u8 = 22;

#[derive(Clone)]
pub struct Encryptor {
    passphrase: Zeroizing<String>,
    work_factor: u8,
}

impl Encryptor {
    #[must_use]
    pub fn with_passphrase(passphrase: &str) -> Encryptor {
        Encryptor {
            passphrase: Zeroizing::new(passphrase.to_owned()),
            work_factor: DEFAULT_WORK_FACTOR,
        }
    }

    #[must_use]
    pub fn work_factor(mut self, work_factor: u8) -> Encryptor {
        self.work_factor = work_factor;
        self
    }

    pub fn wrap<W: Write>(self, mut output: W) -> Result<Writer<W>> {
        let file_key = Zeroizing::new(keys::random::<FILE_KEY_BYTES>()?);
        let stanza = passphrase::wrap(&self.passphrase, self.work_factor, &file_key)?;
        let authenticated = header::authenticated(&[stanza]);
        let mac = keys::header_mac(&file_key, &authenticated);
        output.write_all(&header::render(&authenticated, &mac))?;
        let nonce = keys::random::<PAYLOAD_NONCE_BYTES>()?;
        output.write_all(&nonce)?;
        Ok(Writer::new(output, keys::payload_key(&file_key, &nonce)))
    }
}

impl std::fmt::Debug for Encryptor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Encryptor")
            .field("work_factor", &self.work_factor)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct Decryptor<R: Read> {
    input: BufReader<R>,
    header: Header,
    allowed: u8,
    work_factor: u8,
}

impl<R: Read> Decryptor<R> {
    pub fn new(input: R) -> Result<Decryptor<R>> {
        let mut input = BufReader::new(input);
        let header = Header::read(&mut input)?;
        let scrypt = header
            .stanzas
            .iter()
            .filter(|stanza| stanza.tag == passphrase::TAG)
            .count();
        match (scrypt, header.stanzas.len()) {
            (0, _) => return Err(Error::NotPassphrase),
            (1, 1) => {}
            _ => return Err(Error::Header("an scrypt stanza must be the only stanza")),
        }
        let work_factor = passphrase::work_factor(&header.stanzas[0])?;
        Ok(Decryptor {
            input,
            header,
            allowed: MAX_WORK_FACTOR,
            work_factor,
        })
    }

    #[must_use]
    pub fn work_factor(&self) -> u8 {
        self.work_factor
    }

    #[must_use]
    pub fn max_work_factor(mut self, allowed: u8) -> Decryptor<R> {
        self.allowed = allowed;
        self
    }

    pub fn decrypt(mut self, passphrase: &str) -> Result<Reader<BufReader<R>>> {
        let file_key = passphrase::unwrap(passphrase, &self.header.stanzas[0], self.allowed)?;
        keys::verify_header(&file_key, &self.header.authenticated, &self.header.mac)?;
        let mut nonce = [0u8; PAYLOAD_NONCE_BYTES];
        self.input
            .read_exact(&mut nonce)
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::UnexpectedEof => Error::Truncated,
                _ => Error::Io(error),
            })?;
        Ok(Reader::new(
            self.input,
            keys::payload_key(&file_key, &nonce),
        ))
    }
}

pub fn encrypt(passphrase: &str, plaintext: &[u8]) -> Result<Vec<u8>> {
    let mut writer = Encryptor::with_passphrase(passphrase).wrap(Vec::new())?;
    writer.write_all(plaintext)?;
    Ok(writer.finish()?)
}

pub fn decrypt(passphrase: &str, ciphertext: impl Read) -> Result<Zeroizing<Vec<u8>>> {
    let mut reader = Decryptor::new(ciphertext)?.decrypt(passphrase)?;
    let mut plaintext = Zeroizing::new(Vec::new());
    reader.read_to_end(&mut plaintext).map_err(unwrap_io)?;
    Ok(plaintext)
}

fn unwrap_io(error: io::Error) -> Error {
    if !error.get_ref().is_some_and(|inner| inner.is::<Error>()) {
        return Error::Io(error);
    }
    let kind = error.kind();
    match error.into_inner().map(|inner| inner.downcast::<Error>()) {
        Some(Ok(inner)) => *inner,
        Some(Err(other)) => Error::Io(io::Error::new(kind, other)),
        None => Error::Io(io::Error::from(kind)),
    }
}
