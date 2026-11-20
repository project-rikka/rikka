use rikka_sync::{Mutex, MutexGuard};
use std::pin::pin;
use std::time::Duration;

use async_io::Timer;
use futures_util::StreamExt as _;
use futures_util::future::{Either, select};
use rikka_hifi::{Device, Event, Hifi, OutputState, Refusal};
use rikka_worker::Subscription;
use zbus::blocking::{Connection, Proxy, connection, proxy};
use zbus::fdo::{DBusProxy, NameOwnerChangedStream, RequestNameFlags};
use zbus::proxy::CacheProperties;

use crate::support::wait_for;

const INTERFACE: &str = "org.freedesktop.ReserveDevice1";
const CALL_TIMEOUT: Duration = Duration::from_secs(5);
const TAKEOVER: Duration = Duration::from_secs(2);
pub(crate) const STALLED_REPLY: Duration = Duration::from_secs(20);

static CARD: Mutex<()> = Mutex::new(());

pub(crate) fn lock() -> MutexGuard<'static, ()> {
    CARD.lock()
}

pub(crate) fn device() -> Option<Device> {
    let devices = Hifi::devices().ok()?;
    let mut playable: Vec<Device> = devices
        .into_iter()
        .filter(|device| device.capabilities.is_some())
        .collect();
    playable.sort_by_key(|device| (device.dsd().native.is_empty(), device.mixer.is_none()));
    let chosen = playable.into_iter().next();
    if chosen.is_none() {
        eprintln!("no sound card to claim: the exclusive test is skipped");
    }
    chosen
}

pub(crate) fn claimed(seen: &mut Vec<Event>, events: &mut Subscription<Event>) -> bool {
    let state = wait_for(events, seen, |event| {
        matches!(
            event,
            Event::Output(OutputState::Exclusive(_) | OutputState::FellBack { .. })
        )
    });
    match state {
        Some(Event::Output(OutputState::Exclusive(_))) => true,
        Some(Event::Output(OutputState::FellBack {
            reason: reason @ (Refusal::Busy { .. } | Refusal::Missing),
            ..
        })) => {
            eprintln!("the card was not handed over ({reason}): the exclusive test is skipped");
            false
        }
        other => panic!("the exclusive output answered {other:?}"),
    }
}

pub(crate) fn bus() -> Connection {
    connection::Builder::session()
        .and_then(|builder| builder.method_timeout(CALL_TIMEOUT).build())
        .expect("the session bus answers")
}

pub(crate) fn holder(card: u32) -> Option<String> {
    reserved(&bus(), card)
        .ok()?
        .get_property("ApplicationName")
        .ok()
}

pub(crate) fn ask_release(bus: &Connection, card: u32, priority: i32) -> zbus::Result<bool> {
    reserved(bus, card)?.call("RequestRelease", &(priority,))
}

pub(crate) struct Stall {
    bus: Connection,
    card: u32,
}

impl Drop for Stall {
    fn drop(&mut self) {
        let _ = self.bus.release_name(name(self.card));
    }
}

pub(crate) fn stall(card: u32, application: &str) -> Option<Stall> {
    let bus = bus();
    bus.object_server()
        .at(
            path(card),
            Stalled {
                application: application.to_owned(),
            },
        )
        .ok()?;
    if take(&bus, card, false) {
        return Some(Stall { bus, card });
    }
    let mut owners = OwnerWatch::new(card);
    if !ask_release(&bus, card, i32::MAX).unwrap_or(false) {
        eprintln!("the card's holder kept it: the stalled holder test is skipped");
        return None;
    }
    if owners.wait_until(TAKEOVER, || take(&bus, card, true)) {
        return Some(Stall { bus, card });
    }
    eprintln!("the card's name was not handed over: the stalled holder test is skipped");
    None
}

pub(crate) struct OwnerWatch {
    changes: NameOwnerChangedStream,
}

impl OwnerWatch {
    pub(crate) fn new(card: u32) -> OwnerWatch {
        let bus = bus();
        let changes = async_io::block_on(async {
            DBusProxy::new(bus.inner())
                .await?
                .receive_name_owner_changed_with_args(&[(0, name(card).as_str())])
                .await
        })
        .expect("the session bus accepts a NameOwnerChanged match rule");
        OwnerWatch { changes }
    }

    pub(crate) fn wait_until(&mut self, timeout: Duration, mut done: impl FnMut() -> bool) -> bool {
        let mut deadline = Timer::after(timeout);
        async_io::block_on(async {
            loop {
                if done() {
                    return true;
                }
                let changed = pin!(self.changes.next());
                match select(changed, &mut deadline).await {
                    Either::Left((Some(_), _)) => {}
                    Either::Left((None, _)) | Either::Right(_) => return done(),
                }
            }
        })
    }
}

fn take(bus: &Connection, card: u32, replacing: bool) -> bool {
    let flags = if replacing {
        RequestNameFlags::DoNotQueue | RequestNameFlags::ReplaceExisting
    } else {
        RequestNameFlags::DoNotQueue.into()
    };
    bus.request_name_with_flags(name(card), flags).is_ok()
}

fn reserved(bus: &Connection, card: u32) -> zbus::Result<Proxy<'static>> {
    proxy::Builder::new(bus)
        .destination(name(card))?
        .path(path(card))?
        .interface(INTERFACE)?
        .cache_properties(CacheProperties::No)
        .build()
}

fn name(card: u32) -> String {
    format!("{INTERFACE}.Audio{card}")
}

fn path(card: u32) -> String {
    format!("/org/freedesktop/ReserveDevice1/Audio{card}")
}

struct Stalled {
    application: String,
}

#[zbus::interface(name = "org.freedesktop.ReserveDevice1")]
impl Stalled {
    async fn request_release(&self, priority: i32) -> bool {
        if priority >= 0 {
            Timer::after(STALLED_REPLY).await;
        }
        false
    }

    #[zbus(property)]
    fn priority(&self) -> i32 {
        i32::MAX
    }

    #[zbus(property)]
    fn application_name(&self) -> String {
        self.application.clone()
    }

    #[zbus(property)]
    fn application_device_name(&self) -> String {
        String::new()
    }
}
