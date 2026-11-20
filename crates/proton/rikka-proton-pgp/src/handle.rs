use crate::clock::Clock;
use crate::decryption::Decryption;
use crate::encryption::Encryption;
use crate::key_expiration::KeyExpiration;
use crate::key_generation::KeyGeneration;
use crate::key_revocation::KeyRevocation;
use crate::private_key::PrivateKey;
use crate::signing::Signing;
use crate::verification::Verification;

#[derive(Debug, Clone, Copy, Default)]
pub struct Pgp {
    clock: Clock,
}

impl Pgp {
    #[must_use]
    pub fn new() -> Self {
        Pgp::default()
    }

    #[must_use]
    pub fn with_clock(clock: Clock) -> Self {
        Pgp { clock }
    }

    #[must_use]
    pub fn clock(&self) -> Clock {
        self.clock
    }

    #[must_use]
    pub fn now(&self) -> u64 {
        self.clock.now()
    }

    pub fn generate_key(&self) -> KeyGeneration {
        KeyGeneration::new(self.now())
    }

    pub fn change_expiration(&self, key: &PrivateKey) -> KeyExpiration {
        KeyExpiration::new(key.clone(), self.now())
    }

    pub fn revoke(&self, key: &PrivateKey) -> KeyRevocation {
        KeyRevocation::new(key.clone(), self.now())
    }

    pub fn encrypt(&self) -> Encryption {
        Encryption::new(self.now())
    }

    pub fn decrypt(&self) -> Decryption {
        Decryption::new(self.now())
    }

    pub fn sign(&self) -> Signing {
        Signing::new(self.now())
    }

    pub fn verify(&self) -> Verification {
        Verification::new(self.now())
    }
}
