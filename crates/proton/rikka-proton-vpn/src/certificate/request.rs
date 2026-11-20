use std::time::Duration;

use serde::Serialize;

use crate::certificate::features::CertificateFeatures;
use crate::key::KeyPair;

const PUBLIC_KEY_MODE: &str = "EC";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CertificateMode {
    #[default]
    Session,
    Persistent,
}

#[must_use]
#[derive(Debug, Clone)]
pub struct CertificateRequest {
    public_key: String,
    mode: CertificateMode,
    device_name: Option<String>,
    duration: Option<String>,
    features: CertificateFeatures,
}

#[derive(Debug, Serialize)]
pub(crate) struct CertificateBody<'a> {
    #[serde(rename = "ClientPublicKey")]
    client_public_key: &'a str,
    #[serde(rename = "ClientPublicKeyMode")]
    client_public_key_mode: &'static str,
    #[serde(rename = "Mode")]
    mode: &'static str,
    #[serde(rename = "DeviceName", skip_serializing_if = "Option::is_none")]
    device_name: Option<&'a str>,
    #[serde(rename = "Duration", skip_serializing_if = "Option::is_none")]
    duration: Option<&'a str>,
    #[serde(rename = "Features", skip_serializing_if = "Option::is_none")]
    features: Option<&'a CertificateFeatures>,
}

impl CertificateMode {
    #[must_use]
    pub fn api_name(self) -> &'static str {
        match self {
            CertificateMode::Session => "session",
            CertificateMode::Persistent => "persistent",
        }
    }
}

impl CertificateRequest {
    pub fn new(keys: &KeyPair) -> CertificateRequest {
        CertificateRequest::for_public_key(keys.public_key_pem())
    }

    pub fn for_public_key(pem: impl Into<String>) -> CertificateRequest {
        CertificateRequest {
            public_key: pem.into(),
            mode: CertificateMode::Session,
            device_name: None,
            duration: None,
            features: CertificateFeatures::default(),
        }
    }

    pub fn mode(mut self, mode: CertificateMode) -> CertificateRequest {
        self.mode = mode;
        self
    }

    pub fn device_name(mut self, name: impl Into<String>) -> CertificateRequest {
        self.device_name = Some(name.into());
        self
    }

    pub fn duration(mut self, duration: Duration) -> CertificateRequest {
        self.duration = Some(format!("{} min", duration.as_secs() / 60));
        self
    }

    pub fn features(mut self, features: CertificateFeatures) -> CertificateRequest {
        self.features = features;
        self
    }

    pub(crate) fn body(&self) -> CertificateBody<'_> {
        CertificateBody {
            client_public_key: &self.public_key,
            client_public_key_mode: PUBLIC_KEY_MODE,
            mode: self.mode.api_name(),
            device_name: self.device_name.as_deref(),
            duration: self.duration.as_deref(),
            features: (!self.features.is_empty()).then_some(&self.features),
        }
    }
}
