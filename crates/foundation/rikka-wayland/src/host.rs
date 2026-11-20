use rikka_sync::Mutex;
use std::sync::Arc;

use rikka_wayland_client::Connection;

use crate::display::Display;
use crate::error::Result;
use crate::reader::{self, Reader};
use crate::serials::Serials;

#[derive(Clone)]
pub struct Host {
    display: Display,
    serials: Arc<Mutex<Option<Serials>>>,
    _reader: Option<Arc<Reader>>,
}

impl Host {
    pub fn with_reader(display: Display) -> Result<Host> {
        let reader = reader::start(display.connection())?;
        Ok(Host {
            display,
            serials: Arc::default(),
            _reader: Some(Arc::new(reader)),
        })
    }

    #[must_use]
    pub fn read_by_owner(display: Display) -> Host {
        Host {
            display,
            serials: Arc::default(),
            _reader: None,
        }
    }

    pub fn serials(&self) -> Result<Serials> {
        let mut serials = self.serials.lock();
        if let Some(current) = serials.as_ref() {
            return Ok(current.clone());
        }
        let listening = Serials::listen(&self.display)?;
        *serials = Some(listening.clone());
        Ok(listening)
    }

    pub fn set_serials(&self, installed: Serials) {
        let mut serials = self.serials.lock();
        if serials.is_some() {
            tracing::debug!("input serials already tracked on this display");
            return;
        }
        *serials = Some(installed);
    }

    #[must_use]
    pub fn connection(&self) -> &Connection {
        self.display.connection()
    }

    #[must_use]
    pub fn display(&self) -> &Display {
        &self.display
    }
}

impl std::fmt::Debug for Host {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Host").finish_non_exhaustive()
    }
}
