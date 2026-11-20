use rikka_sync::{Mutex, MutexGuard};
use std::io;
use std::sync::Arc;
use std::time::Duration;

use p256::SecretKey;
use rikka_fido::cbor::Value;
use rikka_fido::{Device, HidLink, Transport, User};

use crate::crypto;
use crate::link::FakeLink;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Ctap21,
    Ctap20,
    U2f,
}

#[derive(Debug, Clone)]
pub(crate) struct Credential {
    pub(crate) application: [u8; 32],
    pub(crate) id: Vec<u8>,
    pub(crate) user: Option<User>,
    pub(crate) protection: u64,
    pub(crate) cred_random: [u8; 32],
    pub(crate) cred_random_uv: [u8; 32],
    pub(crate) large_blob_key: Option<[u8; 32]>,
}

impl Credential {
    pub(crate) fn new(application: [u8; 32], id: Vec<u8>, user: Option<User>) -> Credential {
        let large_blob_key = user.is_some().then(crypto::random::<32>);
        Credential {
            application,
            id,
            user,
            protection: 1,
            cred_random: crypto::random(),
            cred_random_uv: crypto::random(),
            large_blob_key,
        }
    }
}

#[derive(Debug)]
pub(crate) struct State {
    pub(crate) kind: Kind,
    pub(crate) pin_supported: bool,
    pub(crate) pin: Option<String>,
    pub(crate) retries: u32,
    pub(crate) min_pin_length: u64,
    pub(crate) force_pin_change: bool,
    pub(crate) protocols: Vec<u64>,
    pub(crate) max_credentials: Option<u64>,
    pub(crate) touches: usize,
    pub(crate) noisy: bool,
    pub(crate) unanswered: bool,
    pub(crate) touch_delay: Duration,
    pub(crate) credentials: Vec<Credential>,
    pub(crate) large_blobs: Vec<u8>,
    pub(crate) agreement: Option<SecretKey>,
    pub(crate) token: Option<Vec<u8>>,
    pub(crate) counter: u32,
    pub(crate) channels: Vec<u32>,
    pub(crate) waiting: usize,
    pub(crate) busy_sent: bool,
    pub(crate) next: Vec<Value>,
    pub(crate) requests: Vec<(u8, Value)>,
    pub(crate) apdus: Vec<Vec<u8>>,
    pub(crate) permissions: Vec<(u64, String)>,
    pub(crate) cancelled: bool,
}

#[derive(Debug, Clone)]
pub struct FakeKey {
    name: String,
    state: Arc<Mutex<State>>,
}

impl FakeKey {
    #[must_use]
    pub fn new(kind: Kind) -> FakeKey {
        FakeKey {
            name: "fake".to_owned(),
            state: Arc::new(Mutex::new(State {
                kind,
                pin_supported: kind != Kind::U2f,
                pin: None,
                retries: 8,
                min_pin_length: 4,
                force_pin_change: false,
                protocols: match kind {
                    Kind::Ctap21 => vec![2, 1],
                    Kind::Ctap20 | Kind::U2f => vec![1],
                },
                max_credentials: None,
                touches: 2,
                noisy: false,
                unanswered: false,
                touch_delay: Duration::ZERO,
                credentials: Vec::new(),
                large_blobs: crate::large_blobs::serialize(&[]),
                agreement: None,
                token: None,
                counter: 0,
                channels: Vec::new(),
                waiting: 0,
                busy_sent: false,
                next: Vec::new(),
                requests: Vec::new(),
                apdus: Vec::new(),
                permissions: Vec::new(),
                cancelled: false,
            })),
        }
    }

    #[must_use]
    pub fn named(mut self, name: impl Into<String>) -> FakeKey {
        self.name = name.into();
        self
    }

    #[must_use]
    pub fn with_pin(self, pin: impl Into<String>) -> FakeKey {
        let pin = pin.into();
        self.edit(|state| state.pin = Some(pin))
    }

    #[must_use]
    pub fn without_pin_support(self) -> FakeKey {
        self.edit(|state| state.pin_supported = false)
    }

    #[must_use]
    pub fn with_min_pin_length(self, length: u64) -> FakeKey {
        self.edit(|state| state.min_pin_length = length)
    }

    #[must_use]
    pub fn forcing_pin_change(self) -> FakeKey {
        self.edit(|state| state.force_pin_change = true)
    }

    #[must_use]
    pub fn with_protocols(self, protocols: &[u64]) -> FakeKey {
        let protocols = protocols.to_vec();
        self.edit(|state| state.protocols = protocols)
    }

    #[must_use]
    pub fn with_max_credentials(self, count: u64) -> FakeKey {
        self.edit(|state| state.max_credentials = Some(count))
    }

    #[must_use]
    pub fn with_credential(self, rp_id: &str, id: impl Into<Vec<u8>>) -> FakeKey {
        let credential = Credential::new(crypto::sha256(rp_id), id.into(), None);
        self.edit(|state| state.credentials.push(credential))
    }

    #[must_use]
    pub fn with_resident(self, rp_id: &str, id: impl Into<Vec<u8>>, user: User) -> FakeKey {
        let credential = Credential::new(crypto::sha256(rp_id), id.into(), Some(user));
        self.edit(|state| state.credentials.push(credential))
    }

    #[must_use]
    pub fn with_large_blob(self, id: &[u8], blob: &[u8]) -> FakeKey {
        self.edit(|state| {
            let key = state
                .credentials
                .iter()
                .find(|credential| credential.id == id)
                .and_then(|credential| credential.large_blob_key)
                .expect("a discoverable credential with a large-blob key");
            let stranger = crypto::random::<32>();
            state.large_blobs =
                crate::large_blobs::serialize(&[(stranger, b"not yours".as_slice()), (key, blob)]);
        })
    }

    #[must_use]
    pub fn with_touches(self, touches: usize) -> FakeKey {
        self.edit(|state| state.touches = touches)
    }

    #[must_use]
    pub fn noisy(self) -> FakeKey {
        self.edit(|state| state.noisy = true)
    }

    #[must_use]
    pub fn with_touch_delay(self, delay: Duration) -> FakeKey {
        self.edit(|state| state.touch_delay = delay)
    }

    #[must_use]
    pub fn unanswered(self) -> FakeKey {
        self.edit(|state| state.unanswered = true)
    }

    #[must_use]
    pub fn device(&self) -> Device {
        Device::new(format!("fake/{}", self.name))
            .with_name(self.name.clone())
            .with_ids(0x1209, 0x000f)
    }

    #[must_use]
    pub fn requests(&self, command: u8) -> Vec<Value> {
        self.lock()
            .requests
            .iter()
            .filter(|(seen, _)| *seen == command)
            .map(|(_, parameters)| parameters.clone())
            .collect()
    }

    #[must_use]
    pub fn apdus(&self) -> Vec<Vec<u8>> {
        self.lock().apdus.clone()
    }

    #[must_use]
    pub fn permissions(&self) -> Vec<(u64, String)> {
        self.lock().permissions.clone()
    }

    #[must_use]
    pub fn pin(&self) -> Option<String> {
        self.lock().pin.clone()
    }

    #[must_use]
    pub fn pin_retries(&self) -> u32 {
        self.lock().retries
    }

    #[must_use]
    pub fn was_cancelled(&self) -> bool {
        self.lock().cancelled
    }

    #[must_use]
    pub fn credential_ids(&self, rp_id: &str) -> Vec<Vec<u8>> {
        let application = crypto::sha256(rp_id);
        self.lock()
            .credentials
            .iter()
            .filter(|credential| credential.application == application)
            .map(|credential| credential.id.clone())
            .collect()
    }

    #[must_use]
    pub fn cred_random(&self, id: &[u8], verified: bool) -> Option<[u8; 32]> {
        self.lock()
            .credentials
            .iter()
            .find(|credential| credential.id == id)
            .map(|credential| {
                if verified {
                    credential.cred_random_uv
                } else {
                    credential.cred_random
                }
            })
    }

    #[must_use]
    pub fn protection(&self, id: &[u8]) -> Option<u64> {
        self.lock()
            .credentials
            .iter()
            .find(|credential| credential.id == id)
            .map(|credential| credential.protection)
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock()
    }

    fn edit(self, change: impl FnOnce(&mut State)) -> FakeKey {
        change(&mut self.lock());
        self
    }
}

impl Transport for FakeKey {
    fn devices(&self) -> io::Result<Vec<Device>> {
        Ok(vec![self.device()])
    }

    fn open(&self, device: &Device) -> io::Result<Box<dyn HidLink>> {
        if *device != self.device() {
            return Err(io::ErrorKind::NotFound.into());
        }
        Ok(Box::new(FakeLink::new(self.clone())))
    }
}

#[derive(Debug, Clone)]
pub struct FakeKeys(Vec<FakeKey>);

impl FakeKeys {
    #[must_use]
    pub fn new(keys: impl IntoIterator<Item = FakeKey>) -> FakeKeys {
        FakeKeys(keys.into_iter().collect())
    }
}

impl Transport for FakeKeys {
    fn devices(&self) -> io::Result<Vec<Device>> {
        Ok(self.0.iter().map(FakeKey::device).collect())
    }

    fn open(&self, device: &Device) -> io::Result<Box<dyn HidLink>> {
        self.0
            .iter()
            .find(|key| key.device() == *device)
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?
            .open(device)
    }
}
