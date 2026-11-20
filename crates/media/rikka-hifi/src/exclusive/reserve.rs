use rikka_sync::Mutex;
use std::collections::HashMap;
use std::pin::pin;
use std::sync::Arc;
use std::time::Duration;

use async_io::Timer;
use futures_channel::oneshot;
use futures_util::StreamExt as _;
use futures_util::future::{Either, select};
use zbus::blocking::{Connection, Proxy, connection, proxy};
use zbus::fdo::{NameOwnerChangedStream, RequestNameFlags, RequestNameReply};
use zbus::proxy::CacheProperties;

use crate::event::Refusal;
use crate::notice::{Notice, Notifier};

const INTERFACE: &str = "org.freedesktop.ReserveDevice1";
const PRIORITY: i32 = 10;
const VACATE_TIMEOUT: Duration = Duration::from_secs(2);
const CALL_TIMEOUT: Duration = Duration::from_secs(3);
const RELEASE_TIMEOUT: Duration = Duration::from_secs(3);

struct Owner {
    application: String,
    device: String,
    notifier: Notifier,
}

type OwnerSlot = Arc<Mutex<Option<Owner>>>;

pub(crate) struct Bus {
    connection: Connection,
    exported: HashMap<u32, OwnerSlot>,
}

pub(crate) struct Reservation {
    connection: Connection,
    name: String,
    owner_slot: OwnerSlot,
}

struct ReserveObject {
    owner_slot: OwnerSlot,
}

impl Bus {
    pub(crate) fn session() -> Option<Bus> {
        let connected = connection::Builder::session()
            .and_then(|builder| builder.method_timeout(CALL_TIMEOUT).build());
        match connected {
            Ok(connection) => Some(Bus {
                connection,
                exported: HashMap::new(),
            }),
            Err(error) => {
                tracing::debug!(%error, "no session bus for device reservation");
                None
            }
        }
    }

    pub(crate) fn claim(
        &mut self,
        card: u32,
        application: &str,
        device: &str,
        notifier: &Notifier,
    ) -> Result<Reservation, Refusal> {
        let name = format!("{INTERFACE}.Audio{card}");
        let path = format!("/org/freedesktop/ReserveDevice1/Audio{card}");
        let owner_slot = self.export(card, &path)?;
        *owner_slot.lock() = Some(Owner {
            application: application.to_owned(),
            device: device.to_owned(),
            notifier: notifier.clone(),
        });
        let reservation = Reservation {
            connection: self.connection.clone(),
            name,
            owner_slot,
        };
        reservation.acquire(&path)?;
        Ok(reservation)
    }

    fn export(&mut self, card: u32, path: &str) -> Result<OwnerSlot, Refusal> {
        if let Some(owner_slot) = self.exported.get(&card) {
            return Ok(Arc::clone(owner_slot));
        }
        let owner_slot: OwnerSlot = Arc::new(Mutex::new(None));
        self.connection
            .object_server()
            .at(
                path,
                ReserveObject {
                    owner_slot: Arc::clone(&owner_slot),
                },
            )
            .map_err(bus_error)?;
        self.exported.insert(card, Arc::clone(&owner_slot));
        Ok(owner_slot)
    }
}

impl Reservation {
    fn acquire(&self, path: &str) -> Result<(), Refusal> {
        if self.request(false)? {
            return Ok(());
        }
        let owner: Proxy<'_> = proxy::Builder::new(&self.connection)
            .destination(self.name.as_str())
            .and_then(|builder| builder.path(path))
            .and_then(|builder| builder.interface(INTERFACE))
            .and_then(|builder| builder.cache_properties(CacheProperties::No).build())
            .map_err(bus_error)?;
        let holder = owner.get_property::<String>("ApplicationName").ok();
        let changes = self.owner_changes()?;
        let released = match owner.call::<_, _, bool>("RequestRelease", &(PRIORITY,)) {
            Ok(released) => released,
            Err(error) => {
                tracing::debug!(%error, name = %self.name, "RequestRelease failed");
                false
            }
        };
        if !released {
            return Err(Refusal::Busy { holder });
        }
        if self.request(true)? || (vacated(changes) && self.request(true)?) {
            return Ok(());
        }
        Err(Refusal::Busy { holder })
    }

    fn owner_changes(&self) -> Result<NameOwnerChangedStream, Refusal> {
        async_io::block_on(async {
            zbus::fdo::DBusProxy::new(self.connection.inner())
                .await?
                .receive_name_owner_changed_with_args(&[(0, self.name.as_str())])
                .await
        })
        .map_err(bus_error)
    }

    fn request(&self, replacing: bool) -> Result<bool, Refusal> {
        let flags = if replacing {
            RequestNameFlags::DoNotQueue | RequestNameFlags::ReplaceExisting
        } else {
            RequestNameFlags::DoNotQueue.into()
        };
        match self
            .connection
            .request_name_with_flags(self.name.as_str(), flags)
        {
            Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => Ok(true),
            Ok(RequestNameReply::Exists | RequestNameReply::InQueue)
            | Err(zbus::Error::NameTaken) => Ok(false),
            Err(error) => Err(bus_error(error)),
        }
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        *self.owner_slot.lock() = None;
        if let Err(error) = self.connection.release_name(self.name.as_str()) {
            tracing::debug!(%error, name = %self.name, "failed to release device reservation");
        }
    }
}

#[zbus::interface(name = "org.freedesktop.ReserveDevice1")]
impl ReserveObject {
    async fn request_release(&self, priority: i32) -> bool {
        let notifier = match &*self.owner_slot.lock() {
            Some(claim) if priority > PRIORITY => claim.notifier.clone(),
            _ => return false,
        };
        let (released, waited) = oneshot::channel();
        notifier.send(Notice::ReleaseRequested(released));
        match select(waited, Timer::after(RELEASE_TIMEOUT)).await {
            Either::Left(_) => true,
            Either::Right(_) => {
                tracing::warn!("device release timed out, keeping the reservation");
                false
            }
        }
    }

    #[zbus(property)]
    fn priority(&self) -> i32 {
        PRIORITY
    }

    #[zbus(property)]
    fn application_name(&self) -> String {
        self.owner_slot
            .lock()
            .as_ref()
            .map(|claim| claim.application.clone())
            .unwrap_or_default()
    }

    #[zbus(property)]
    fn application_device_name(&self) -> String {
        self.owner_slot
            .lock()
            .as_ref()
            .map(|claim| claim.device.clone())
            .unwrap_or_default()
    }
}

fn bus_error(error: zbus::Error) -> Refusal {
    Refusal::Open(error.to_string())
}

fn vacated(changes: NameOwnerChangedStream) -> bool {
    async_io::block_on(async {
        let mut changes = changes;
        let vacated = pin!(async {
            while let Some(change) = changes.next().await {
                if change.args().is_ok_and(|args| args.new_owner().is_none()) {
                    return true;
                }
            }
            false
        });
        match select(vacated, Timer::after(VACATE_TIMEOUT)).await {
            Either::Left((vacated, _)) => vacated,
            Either::Right(_) => false,
        }
    })
}

impl std::fmt::Debug for Reservation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reservation")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}
