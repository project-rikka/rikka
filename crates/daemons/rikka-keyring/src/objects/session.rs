use std::sync::Arc;

use zbus::message::Header;
use zbus::zvariant::OwnedObjectPath;

use crate::fault::Fault;
use crate::objects::caller;
use crate::shared::Shared;

pub(crate) struct SessionObject {
    shared: Arc<Shared>,
    path: OwnedObjectPath,
}

impl SessionObject {
    pub(crate) fn new(shared: Arc<Shared>, path: OwnedObjectPath) -> SessionObject {
        SessionObject { shared, path }
    }
}

#[zbus::interface(name = "org.freedesktop.Secret.Session")]
impl SessionObject {
    async fn close(&self, #[zbus(header)] header: Header<'_>) -> Result<(), Fault> {
        let caller = caller(&header)?;
        let path = self.path.clone();
        self.shared
            .run(move |state| state.close_session(&path, &caller))
            .await
    }
}
