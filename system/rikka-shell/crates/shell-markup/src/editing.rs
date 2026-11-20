use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use rikka_image::Image;
use rikka_sync::Mutex;
use serde::{Deserialize, Serialize};
use shell_notifications::ActionRequest;

use crate::service::{CAPABILITY, EDIT};

#[derive(Clone, Debug, Default)]
pub struct Editing(Arc<Inner>);

#[derive(Debug, Default)]
struct Inner {
    latest: Mutex<Option<Shot>>,
    minted: AtomicU64,
}

#[derive(Debug)]
struct Shot {
    token: u64,
    picture: Arc<Image>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Opening {
    pub(crate) token: u64,
    pub(crate) file: PathBuf,
    pub(crate) quality: u8,
}

impl Editing {
    #[must_use]
    pub fn new() -> Editing {
        Editing::default()
    }

    #[must_use]
    pub fn keep(&self, unsaved: Option<Arc<Image>>, file: PathBuf, quality: u8) -> ActionRequest {
        let token = self
            .0
            .minted
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1);
        *self.0.latest.lock() = unsaved.map(|picture| Shot { token, picture });
        ActionRequest::new(CAPABILITY, EDIT).encode(&Opening {
            token,
            file,
            quality,
        })
    }

    #[must_use]
    pub(crate) fn picture(&self, token: u64) -> Option<Arc<Image>> {
        let latest_shot = self.0.latest.lock();
        let shot = latest_shot.as_ref().filter(|shot| shot.token == token)?;
        Some(Arc::clone(&shot.picture))
    }
}
