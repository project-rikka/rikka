use std::sync::Arc;
use std::time::Duration;

use music_spotify_wire::{Credentials, DeviceId, FileId, Product, SpotifyId, StoredCredentials};
use rikka_sync::Mutex;
use tokio::sync::watch;
use tokio::time::timeout;

use crate::access_points;
use crate::error::{Error, Result};
use crate::link::{AudioKey, Link, Profile, open};

const PRODUCT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub username: String,
    pub country: Option<String>,
    pub product: Product,
}

#[derive(Clone)]
pub struct Session(Arc<Inner>);

struct Inner {
    device: DeviceId,
    resolver: rikka_net::Client,
    credentials: Mutex<StoredCredentials>,
    profile: watch::Sender<Profile>,
    link: tokio::sync::Mutex<Option<Arc<Link>>>,
}

impl Session {
    pub async fn sign_in(access_token: &str, device: DeviceId) -> Result<Session> {
        let resolver = access_points::resolver();
        let points = access_points::resolve(&resolver).await;
        let token = Credentials::AccessToken(access_token.to_owned());
        let stored = open(&points, &token, &device).await?.credentials;
        Session::start(resolver, stored, device).await
    }

    pub async fn connect(credentials: StoredCredentials, device: DeviceId) -> Result<Session> {
        Session::start(access_points::resolver(), credentials, device).await
    }

    async fn start(
        resolver: rikka_net::Client,
        credentials: StoredCredentials,
        device: DeviceId,
    ) -> Result<Session> {
        let session = Session(Arc::new(Inner {
            device,
            resolver,
            credentials: Mutex::new(credentials),
            profile: watch::Sender::new(Profile::default()),
            link: tokio::sync::Mutex::new(None),
        }));
        let link = session.link().await?;
        let mut profile = session.0.profile.subscribe();
        let known = async move {
            profile
                .wait_for(|profile| profile.product.is_some())
                .await
                .is_ok()
        };
        tokio::select! {
            known = timeout(PRODUCT_TIMEOUT, known) => match known {
                Ok(true) => Ok(session),
                Ok(false) => Err(Error::Disconnected),
                Err(_) => Err(Error::TimedOut),
            },
            () = link.closed() => Err(Error::Disconnected),
        }
    }

    #[must_use]
    pub fn credentials(&self) -> StoredCredentials {
        self.0.credentials.lock().clone()
    }

    #[must_use]
    pub fn device(&self) -> &DeviceId {
        &self.0.device
    }

    #[must_use]
    pub fn account(&self) -> Account {
        let profile = self.0.profile.borrow();
        Account {
            username: self.0.credentials.lock().username.clone(),
            country: profile.country.clone(),
            product: profile
                .product
                .clone()
                .expect("a started session has received its product"),
        }
    }

    pub async fn audio_key(&self, track: SpotifyId, file: FileId) -> Result<AudioKey> {
        let link = self.link().await?;
        match link.audio_key(track, file).await {
            Err(Error::Disconnected | Error::TimedOut) => {
                self.retire(&link).await;
                self.link().await?.audio_key(track, file).await
            }
            answer => answer,
        }
    }

    async fn link(&self) -> Result<Arc<Link>> {
        let mut current = self.0.link.lock().await;
        if let Some(link) = current.as_ref().filter(|link| link.is_open()) {
            return Ok(Arc::clone(link));
        }
        let points = access_points::resolve(&self.0.resolver).await;
        let credentials = Credentials::Stored(self.credentials());
        let opened = open(&points, &credentials, &self.0.device).await?;
        *self.0.credentials.lock() = opened.credentials.clone();
        let link = Arc::new(Link::start(opened, self.0.profile.clone()));
        *current = Some(Arc::clone(&link));
        Ok(link)
    }

    async fn retire(&self, link: &Arc<Link>) {
        let mut current = self.0.link.lock().await;
        if current
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, link))
        {
            *current = None;
        }
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Session")
            .field("device", &self.0.device)
            .finish_non_exhaustive()
    }
}
