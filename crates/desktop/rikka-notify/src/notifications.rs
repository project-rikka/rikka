use std::sync::Arc;
use std::time::Duration;

use futures_channel::oneshot;
use futures_util::{StreamExt, future};
use tokio::sync::OnceCell;
use zbus::proxy::{CacheProperties, SignalStream};
use zbus::{Connection, Proxy};

use crate::error::{Error, Result};
use crate::event::Id;
use crate::hub::{Events, Hub};
use crate::notification::Notification;
use crate::wire;

const METHOD_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Default)]
pub struct Notifications {
    inner: Arc<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    link: OnceCell<Link>,
    hub: Arc<Hub>,
}

#[derive(Debug)]
struct Link {
    proxy: Proxy<'static>,
    _listening: oneshot::Sender<()>,
}

impl Notifications {
    #[must_use]
    pub fn new() -> Notifications {
        Notifications::default()
    }

    pub async fn notify(&self, notification: &Notification) -> Result<Id> {
        let link = self.link().await?;
        let raw: u32 = link
            .proxy
            .call("Notify", &wire::notify(notification))
            .await
            .map_err(Error::from_call)?;
        let id = Id::new(raw)
            .ok_or_else(|| Error::Server("server returned notification id 0".to_owned()))?;
        self.inner.hub.claim(id);
        Ok(id)
    }

    pub async fn close(&self, id: Id) -> Result<()> {
        let link = self.link().await?;
        link.proxy
            .call::<_, _, ()>("CloseNotification", &id.get())
            .await
            .map_err(Error::from_call)
    }

    #[must_use]
    pub fn events(&self) -> Events {
        self.inner.hub.listen()
    }

    async fn link(&self) -> Result<&Link> {
        self.inner
            .link
            .get_or_try_init(|| Link::open(Arc::clone(&self.inner.hub)))
            .await
    }
}

impl Link {
    async fn open(hub: Arc<Hub>) -> Result<Link> {
        let connection = zbus::connection::Builder::session()
            .map_err(Error::NoSession)?
            .method_timeout(METHOD_TIMEOUT)
            .build()
            .await
            .map_err(Error::NoSession)?;
        let proxy = proxy(&connection).await.map_err(Error::from_call)?;
        let signals = proxy
            .receive_all_signals()
            .await
            .map_err(Error::from_call)?;
        let (listening, stopped) = oneshot::channel();
        std::thread::Builder::new()
            .name("rikka-notify".to_owned())
            .spawn(move || listen(signals, &hub, stopped))
            .map_err(Error::Spawn)?;
        Ok(Link {
            proxy,
            _listening: listening,
        })
    }
}

async fn proxy(connection: &Connection) -> zbus::Result<Proxy<'static>> {
    zbus::proxy::Builder::new(connection)
        .destination(wire::NAME)?
        .path(wire::PATH)?
        .interface(wire::NAME)?
        .cache_properties(CacheProperties::No)
        .build()
        .await
}

fn listen(signals: SignalStream<'static>, hub: &Hub, stopped: oneshot::Receiver<()>) {
    let dispatching = signals
        .filter_map(|message| future::ready(wire::event_of(&message)))
        .for_each(|event| {
            hub.dispatch(event);
            future::ready(())
        });
    rikka_worker::block_on(future::select(Box::pin(dispatching), stopped));
}
