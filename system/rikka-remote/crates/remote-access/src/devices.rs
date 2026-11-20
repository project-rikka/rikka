use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

use sha2::{Digest, Sha256};

const TOKEN_BYTES: usize = 32;
const MOST_DEVICES: usize = 32;
const UNUSED_FOR: u64 = 90 * 24 * 60 * 60;

#[derive(Clone, PartialEq, Eq)]
pub struct Token([u8; TOKEN_BYTES]);

impl Token {
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl std::fmt::Debug for Token {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Token")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    digest: [u8; 32],
    device: String,
    created: u64,
    used: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Devices {
    entries: Vec<Entry>,
}

impl Devices {
    pub fn load(path: &Path) -> io::Result<Devices> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Devices::default()),
            Err(error) => return Err(error),
        };
        let entries = text.lines().filter_map(parse).collect();
        Ok(Devices { entries })
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let mut file = rikka_fs::Replace::new(path).private().stage()?;
        for entry in &self.entries {
            let device: String = entry.device.chars().filter(|c| !c.is_control()).collect();
            writeln!(
                file,
                "{} {} {} {device}",
                rikka_types::hex::encode(entry.digest),
                entry.created,
                entry.used
            )?;
        }
        file.commit()
    }

    pub fn remember(&mut self, device: &str, now: u64) -> io::Result<Token> {
        let mut secret = [0_u8; TOKEN_BYTES];
        getrandom::fill(&mut secret).map_err(io::Error::other)?;
        self.prune(now);
        while self.entries.len() >= MOST_DEVICES {
            let oldest = self
                .entries
                .iter()
                .enumerate()
                .min_by_key(|(_, entry)| entry.used)
                .map_or(0, |(index, _)| index);
            self.entries.remove(oldest);
        }
        self.entries.push(Entry {
            digest: Sha256::digest(secret).into(),
            device: device.to_owned(),
            created: now,
            used: now,
        });
        Ok(Token(secret))
    }

    pub fn recall(&mut self, token: &[u8], now: u64) -> Option<String> {
        self.prune(now);
        let digest: [u8; 32] = Sha256::digest(token).into();
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.digest == digest)?;
        entry.used = now;
        Some(entry.device.clone())
    }

    pub fn forget(&mut self, token: &[u8]) -> bool {
        let digest: [u8; 32] = Sha256::digest(token).into();
        let before = self.entries.len();
        self.entries.retain(|entry| entry.digest != digest);
        self.entries.len() != before
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn prune(&mut self, now: u64) {
        self.entries
            .retain(|entry| now.saturating_sub(entry.used) < UNUSED_FOR);
    }
}

fn parse(line: &str) -> Option<Entry> {
    let mut fields = line.splitn(4, ' ');
    let mut digest = [0_u8; 32];
    rikka_types::hex::decode_to_slice(fields.next()?, &mut digest).ok()?;
    let created = fields.next()?.parse().ok()?;
    let used = fields.next()?.parse().ok()?;
    let device = fields.next().unwrap_or_default().to_owned();
    Some(Entry {
        digest,
        device,
        created,
        used,
    })
}
