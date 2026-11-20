use rikka_sync::Mutex;
use std::sync::Arc;
use std::time::Duration;

use jiff::Timestamp;

use crate::certificate::Certificate;
use crate::client::VpnClient;
use crate::connect::settings::ConnectionSettings;
use crate::error::Result;
use crate::key::KeyPair;

const MIN_RETRY_DELAY: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct Credentials {
    keys: KeyPair,
    certificate: Option<Certificate>,
}

#[derive(Debug, Clone)]
pub(crate) struct Keyring(Arc<Mutex<Credentials>>);

impl Credentials {
    #[must_use]
    pub fn new(keys: KeyPair) -> Credentials {
        Credentials {
            keys,
            certificate: None,
        }
    }

    #[must_use]
    pub fn generate() -> Credentials {
        Credentials::new(KeyPair::generate())
    }

    #[must_use]
    pub fn with_certificate(self, certificate: Certificate) -> Credentials {
        Credentials {
            certificate: Some(certificate),
            ..self
        }
    }

    #[must_use]
    pub fn keys(&self) -> &KeyPair {
        &self.keys
    }

    #[must_use]
    pub fn certificate(&self) -> Option<&Certificate> {
        self.certificate.as_ref()
    }
}

impl Keyring {
    pub(crate) fn new(credentials: Credentials) -> Keyring {
        Keyring(Arc::new(Mutex::new(credentials)))
    }

    pub(crate) fn current_credentials(&self) -> Credentials {
        self.0.lock().clone()
    }

    pub(crate) async fn valid(
        &self,
        client: &VpnClient,
        settings: &ConnectionSettings,
    ) -> Result<(KeyPair, Certificate)> {
        let current_credentials = self.current_credentials();
        match current_credentials.certificate {
            Some(certificate) if !certificate.freshness().needs_refresh() => {
                Ok((current_credentials.keys, certificate))
            }
            _ => self.fetch(client, settings, current_credentials.keys).await,
        }
    }

    pub(crate) async fn replacement_for(
        &self,
        client: &VpnClient,
        settings: &ConnectionSettings,
        used: &Certificate,
    ) -> Result<(KeyPair, Certificate)> {
        let current_credentials = self.current_credentials();
        match current_credentials.certificate {
            Some(certificate)
                if certificate.certificate != used.certificate
                    && !certificate.freshness().is_expired() =>
            {
                Ok((current_credentials.keys, certificate))
            }
            _ => self.fetch(client, settings, current_credentials.keys).await,
        }
    }

    pub(crate) async fn renew_key(
        &self,
        client: &VpnClient,
        settings: &ConnectionSettings,
    ) -> Result<(KeyPair, Certificate)> {
        self.fetch(client, settings, KeyPair::generate()).await
    }

    async fn fetch(
        &self,
        client: &VpnClient,
        settings: &ConnectionSettings,
        keys: KeyPair,
    ) -> Result<(KeyPair, Certificate)> {
        let certificate = client
            .certificate(&settings.certificate_request(&keys))
            .await?;
        let mut current_credentials = self.0.lock();
        *current_credentials = Credentials {
            keys: keys.clone(),
            certificate: Some(certificate.clone()),
        };
        Ok((keys, certificate))
    }
}

pub(crate) fn retry_after_failure(certificate: &Certificate, now: Timestamp) -> Duration {
    (certificate.time_until_expiry_at(now) / 2).max(MIN_RETRY_DELAY)
}
