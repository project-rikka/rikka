use std::fmt;
use std::sync::Arc;

use rikka_sync::RwLock;

use crate::error::Result;
use crate::session::Session;

pub trait SessionStore: fmt::Debug + Send + Sync + 'static {
    fn load(&self) -> Result<Option<Session>>;

    fn save(&self, session: &Session) -> Result<()>;

    fn remove(&self) -> Result<()>;
}

impl<T: SessionStore + ?Sized> SessionStore for Arc<T> {
    fn load(&self) -> Result<Option<Session>> {
        (**self).load()
    }

    fn save(&self, session: &Session) -> Result<()> {
        (**self).save(session)
    }

    fn remove(&self) -> Result<()> {
        (**self).remove()
    }
}

#[derive(Debug, Default)]
pub struct MemorySessionStore {
    session: RwLock<Option<Session>>,
}

impl MemorySessionStore {
    #[must_use]
    pub fn new() -> MemorySessionStore {
        MemorySessionStore::default()
    }

    #[must_use]
    pub fn with_session(session: Session) -> MemorySessionStore {
        MemorySessionStore {
            session: RwLock::new(Some(session)),
        }
    }
}

impl SessionStore for MemorySessionStore {
    fn load(&self) -> Result<Option<Session>> {
        Ok(self.session.read().clone())
    }

    fn save(&self, session: &Session) -> Result<()> {
        *self.session.write() = Some(session.clone());
        Ok(())
    }

    fn remove(&self) -> Result<()> {
        *self.session.write() = None;
        Ok(())
    }
}
