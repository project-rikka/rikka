use rikka_sync::Mutex;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use remote_access::{Devices, Throttle};
use rikka_remote_wire::{Refusal, SignIn};

use crate::identity::Identity;
use crate::notices::Notices;
use crate::pam::{self, Verdict};

#[derive(Debug)]
pub(crate) enum Admission {
    Granted {
        token: Vec<u8>,
    },
    Refused {
        reason: Refusal,
        retry_after: Duration,
        detail: String,
    },
}

#[derive(Debug)]
pub(crate) struct Access {
    user: String,
    path: PathBuf,
    devices: Mutex<Devices>,
    throttle: Mutex<Throttle>,
    serial: tokio::sync::Mutex<()>,
    saving: tokio::sync::Mutex<()>,
    notices: Notices,
}

impl Access {
    pub(crate) fn load(identity: &Identity, notices: Notices) -> Access {
        let path = identity.devices();
        let devices = Devices::load(&path).unwrap_or_else(|error| {
            tracing::warn!("cannot read the remembered devices: {error}");
            Devices::default()
        });
        Access {
            user: identity.user.clone(),
            path,
            devices: Mutex::new(devices),
            throttle: Mutex::new(Throttle::default()),
            serial: tokio::sync::Mutex::new(()),
            saving: tokio::sync::Mutex::new(()),
            notices,
        }
    }

    pub(crate) async fn admit(&self, request: SignIn, device: &str, from: &str) -> Admission {
        let _serial = self.serial.lock().await;
        if let Some(wait) = self.throttle.lock().wait(Instant::now()) {
            return Admission::Refused {
                reason: Refusal::Throttled,
                retry_after: wait,
                detail: String::new(),
            };
        }
        match request {
            SignIn::Token(token) => {
                let (known, changed) = {
                    let mut devices = self.devices.lock();
                    let before = devices.len();
                    let known = devices.recall(&token, now());
                    let changed = known.is_some() || devices.len() != before;
                    (known, changed)
                };
                if changed {
                    self.store().await;
                }
                match known {
                    Some(_) => {
                        tracing::info!(from, device, "remote viewer resumed");
                        Admission::Granted { token: Vec::new() }
                    }
                    None => Admission::Refused {
                        reason: Refusal::UnknownToken,
                        retry_after: Duration::ZERO,
                        detail: String::new(),
                    },
                }
            }
            SignIn::Password { password, remember } => {
                match pam::verify(&self.user, password).await {
                    Verdict::Granted => {
                        self.throttle.lock().succeeded();
                        tracing::info!(from, device, "remote sign-in");
                        if !remember {
                            return Admission::Granted { token: Vec::new() };
                        }
                        let made = self.devices.lock().remember(device, now());
                        self.store().await;
                        match made {
                            Ok(token) => Admission::Granted {
                                token: token.as_bytes().to_vec(),
                            },
                            Err(error) => {
                                tracing::warn!("cannot remember the device: {error}");
                                Admission::Granted { token: Vec::new() }
                            }
                        }
                    }
                    Verdict::Denied(reason) => {
                        tracing::warn!(from, reason, "remote sign-in refused");
                        self.throttle.lock().failed(Instant::now());
                        self.notices.refused(from);
                        Admission::Refused {
                            reason: Refusal::WrongPassword,
                            retry_after: Duration::ZERO,
                            detail: reason,
                        }
                    }
                    Verdict::Unavailable(reason) => {
                        tracing::error!(reason, "cannot check the password");
                        Admission::Refused {
                            reason: Refusal::Unavailable,
                            retry_after: Duration::ZERO,
                            detail: reason,
                        }
                    }
                }
            }
        }
    }

    pub(crate) async fn forget(&self, token: &[u8]) {
        let forgotten = self.devices.lock().forget(token);
        if forgotten {
            self.store().await;
        }
    }

    async fn store(&self) {
        let _saving = self.saving.lock().await;
        let snapshot = self.devices.lock().clone();
        let path = self.path.clone();
        match tokio::task::spawn_blocking(move || snapshot.save(&path)).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => tracing::warn!("cannot keep the remembered devices: {error}"),
            Err(error) => tracing::warn!("the device store stopped: {error}"),
        }
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}
