use std::marker::PhantomData;
use std::sync::Arc;

use rikka_llm::wire::Transport;
use rikka_net::{HeaderMap, Url};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::dyn_adapter::Access;
use crate::error::{Error, Result};
use crate::renewing::{Credentials, SharedTokens, renewing};

pub struct Auth<'a, T> {
    access: &'a Access<'a>,
    tokens: PhantomData<fn() -> T>,
}

impl<'a, T: Serialize + DeserializeOwned + 'static> Auth<'a, T> {
    pub(crate) fn new(access: &'a Access<'a>) -> Auth<'a, T> {
        Auth {
            access,
            tokens: PhantomData,
        }
    }

    pub fn key(&self) -> Result<String> {
        match self.access.key() {
            "" => Err(Error::Keyless),
            key => Ok(key.to_owned()),
        }
    }

    pub fn tokens(&self) -> Result<T> {
        self.access
            .tokens()
            .and_then(|tokens| T::deserialize(tokens).ok())
            .ok_or(Error::SignedOut)
    }
}

impl<T: Credentials + Serialize + DeserializeOwned> Auth<'_, T> {
    pub fn transport(&self, endpoint: Url, headers: HeaderMap) -> Result<Transport> {
        self.tokens()?;
        let signed_in = self.access.tokens().cloned().ok_or(Error::SignedOut)?;
        let shared = match self.access.shared() {
            Some(shared) => {
                shared.offer(signed_in);
                shared
            }
            None => Arc::new(SharedTokens::private(signed_in)),
        };
        Ok(renewing::<T>(endpoint, headers, shared)?)
    }
}
impl<T> std::fmt::Debug for Auth<'_, T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Auth").finish_non_exhaustive()
    }
}
