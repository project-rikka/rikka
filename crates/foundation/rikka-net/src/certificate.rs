use std::io::Cursor;

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Certificate {
    der: Vec<u8>,
}

impl Certificate {
    pub fn from_der(der: impl Into<Vec<u8>>) -> Result<Certificate> {
        let der = der.into();
        reqwest::Certificate::from_der(&der).map_err(|error| Error::Tls(Box::new(error)))?;
        Ok(Certificate { der })
    }

    pub fn from_pem(pem: &[u8]) -> Result<Certificate> {
        Certificate::bundle_from_pem(pem)?
            .into_iter()
            .next()
            .ok_or_else(|| Error::Tls("no certificate in PEM input".into()))
    }

    pub fn bundle_from_pem(pem: &[u8]) -> Result<Vec<Certificate>> {
        rustls_pemfile::certs(&mut Cursor::new(pem))
            .map(|entry| match entry {
                Ok(der) => Certificate::from_der(der.as_ref()),
                Err(error) => Err(Error::Tls(Box::new(error))),
            })
            .collect()
    }

    #[must_use]
    pub fn as_der(&self) -> &[u8] {
        &self.der
    }

    pub(crate) fn to_reqwest(&self) -> Result<reqwest::Certificate> {
        reqwest::Certificate::from_der(&self.der).map_err(|error| Error::Tls(Box::new(error)))
    }
}
