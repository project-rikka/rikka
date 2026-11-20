use std::sync::Arc;

use jiff::Timestamp;
use rikka_proton_authenticator::{MigrationPayload, Otp, SyncOutcome};

use crate::error::{Error, Result};
use crate::stored::{State, StoredCode};

const MIGRATION: &str = "otpauth-migration://";
const ID_BYTES: usize = 16;

#[derive(Debug, Default)]
pub struct CodeStore {
    codes: Vec<StoredCode>,
    filtered: Arc<Vec<StoredCode>>,
    keyword: String,
}

impl CodeStore {
    #[must_use]
    pub fn new() -> CodeStore {
        CodeStore::default()
    }

    #[must_use]
    pub fn all(&self) -> &[StoredCode] {
        &self.codes
    }

    #[must_use]
    pub fn filter(&self) -> &str {
        &self.keyword
    }
    pub fn set_filter(&mut self, keyword: impl Into<String>) {
        self.keyword = keyword.into();
        self.refilter();
    }

    #[must_use]
    pub fn filtered(&self) -> &Arc<Vec<StoredCode>> {
        &self.filtered
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&StoredCode> {
        self.codes.iter().find(|code| code.id == id)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.filtered().is_empty()
    }

    #[must_use]
    pub fn pending(&self) -> Vec<StoredCode> {
        self.codes
            .iter()
            .filter(|code| code.state != State::Synced)
            .cloned()
            .collect()
    }

    pub fn recall(&mut self, synced: Vec<StoredCode>, pending: Vec<StoredCode>) {
        let mut recalled = synced;
        for code in pending {
            match recalled.iter().position(|synced| synced.id == code.id) {
                Some(at) => recalled[at] = code,
                None => recalled.push(code),
            }
        }
        for code in recalled {
            if self.get(&code.id).is_none() {
                self.codes.push(code);
            }
        }
        self.refilter();
    }

    pub fn import(&mut self, link: &str, note: Option<String>, at: Timestamp) -> Result<usize> {
        let trimmed = link.trim();
        let mut added = 0;

        if trimmed.starts_with(MIGRATION) {
            let payload = MigrationPayload::from_uri(trimmed)?;
            for otp in payload.otps().into_iter().flatten() {
                let position = self.next_position();
                self.put(StoredCode::new(new_id(), &otp, note.clone(), at, position));
                added += 1;
            }
            if added == 0 {
                return Err(Error::NotACode);
            }
            self.refilter();
            return Ok(added);
        }

        self.add(&Otp::parse(trimmed)?, note, at);
        Ok(1)
    }

    pub fn add(&mut self, otp: &Otp, note: Option<String>, at: Timestamp) {
        let position = self.next_position();
        self.put(StoredCode::new(new_id(), otp, note, at, position));
        self.refilter();
    }

    pub fn change(
        &mut self,
        id: &str,
        otp: &Otp,
        note: Option<String>,
        at: Timestamp,
    ) -> Result<()> {
        let Some(code) = self.codes.iter_mut().find(|code| code.id == id) else {
            return Err(Error::NotACode);
        };

        code.otp = otp.clone();
        code.note = note;
        code.state = State::Pending;
        code.edited = Some(at);
        self.refilter();
        Ok(())
    }

    pub fn remove(&mut self, id: &str) {
        let Some(at) = self.codes.iter().position(|code| code.id == id) else {
            return;
        };
        match self.codes[at].remote.is_some() {
            true => {
                self.codes[at].state = State::PendingDelete;
            }
            false => {
                self.codes.remove(at);
            }
        }
        self.refilter();
    }

    pub fn apply_sync(&mut self, outcome: &SyncOutcome) {
        for placement in &outcome.order {
            if let Some(code) = self.codes.iter_mut().find(|code| code.id == placement.id) {
                code.position = placement.position;
                code.modified = placement.modified_at;
            }
        }
        for entry in &outcome.entries {
            self.put(StoredCode::from_remote(entry));
        }
        self.codes
            .retain(|code| !outcome.removed.contains(&code.id));
        for code in &mut self.codes {
            if code.state == State::Pending {
                code.state = State::Synced;
                code.edited = None;
            }
        }
        self.refilter();
    }

    fn put(&mut self, code: StoredCode) {
        match self.codes.iter().position(|cached| cached.id == code.id) {
            Some(at) => self.codes[at] = code,
            None => self.codes.push(code),
        }
    }

    fn next_position(&self) -> u32 {
        self.codes
            .iter()
            .map(|code| code.position + 1)
            .max()
            .unwrap_or_default()
    }

    fn refilter(&mut self) {
        let mut filtered: Vec<StoredCode> = self
            .codes
            .iter()
            .filter(|code| code.state != State::PendingDelete && code.matches(&self.keyword))
            .cloned()
            .collect();
        filtered.sort_by(|one, other| {
            rikka_format::natural_cmp(one.title(), other.title())
                .then_with(|| rikka_format::natural_cmp(one.label(), other.label()))
        });
        self.filtered = Arc::new(filtered);
    }
}

fn new_id() -> String {
    rikka_proton_pgp::random_hex(ID_BYTES)
}
