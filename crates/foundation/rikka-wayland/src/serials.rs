use rikka_sync::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::task::Waker;

use rikka_wayland_client::wl_keyboard::{self, KeyState, WlKeyboard};
use rikka_wayland_client::wl_pointer::{self, ButtonState, WlPointer};
use rikka_wayland_client::wl_seat::{self, Capability, WlSeat};
use rikka_wayland_client::{Connection, Route};

use crate::display::Display;
use crate::error::Result;

const RELEASE_SINCE_VERSION: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SerialKind {
    KeyboardEntered,
    KeyboardLeft,
    KeyPressed,
    KeyReleased,
    ModifiersChanged,
    PointerEntered,
    PointerLeft,
    ButtonPressed,
    ButtonReleased,
}

impl SerialKind {
    pub const ALL: [SerialKind; 9] = [
        SerialKind::KeyboardEntered,
        SerialKind::KeyboardLeft,
        SerialKind::KeyPressed,
        SerialKind::KeyReleased,
        SerialKind::ModifiersChanged,
        SerialKind::PointerEntered,
        SerialKind::PointerLeft,
        SerialKind::ButtonPressed,
        SerialKind::ButtonReleased,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Serial {
    pub serial: u32,
    pub kind: SerialKind,
}

#[derive(Clone)]
pub struct Serials {
    inner: Arc<SerialsInner>,
}

impl Serials {
    pub(crate) fn listen(display: &Display) -> Result<Serials> {
        let log = Arc::new(SerialLog::default());
        let devices = Arc::new(Mutex::new(Devices::default()));
        let listener = SeatListener {
            log: Arc::clone(&log),
            devices: Arc::clone(&devices),
        };
        let route = Route::tap::<WlSeat>(move |seat, event| listener.event(&seat, event));
        let seat: WlSeat = display.bind_on(&route, 1..=crate::display::SEAT_VERSION)?;
        display.flush()?;
        Ok(Serials {
            inner: Arc::new(SerialsInner {
                log,
                devices,
                seat: Mutex::new(Some(seat)),
                owned: Some(display.connection().clone()),
            }),
        })
    }

    #[must_use]
    pub fn recorded() -> (Serials, Recorder) {
        let inner = Arc::new(SerialsInner {
            log: Arc::default(),
            devices: Arc::default(),
            seat: Mutex::new(None),
            owned: None,
        });
        (
            Serials {
                inner: Arc::clone(&inner),
            },
            Recorder(inner),
        )
    }

    #[must_use]
    pub fn latest(&self, among: &[SerialKind]) -> Option<Serial> {
        self.inner.log.latest(among)
    }

    #[must_use]
    pub fn seat(&self) -> Option<WlSeat> {
        self.inner.seat.lock().clone()
    }

    pub fn register_waker(&self, kind: SerialKind, waker: Waker) {
        self.inner.log.waiting.lock().push((kind, waker));
    }
}

impl std::fmt::Debug for Serials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Serials")
            .field("latest", &self.inner.log.latest(&SerialKind::ALL))
            .finish_non_exhaustive()
    }
}

pub struct Recorder(Arc<SerialsInner>);

impl Recorder {
    pub fn follow(&self, seat: Option<WlSeat>) {
        let mut followed = self.0.seat.lock();
        if *followed == seat {
            return;
        }
        *followed = seat;
        self.0.log.clear();
    }

    pub fn record(&self, seat: &WlSeat, kind: SerialKind, serial: u32) {
        let followed = self.0.seat.lock();
        if followed.as_ref() == Some(seat) {
            self.0.log.record(kind, serial);
        }
    }
}

impl std::fmt::Debug for Recorder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Recorder").finish_non_exhaustive()
    }
}

struct SerialsInner {
    log: Arc<SerialLog>,
    devices: Arc<Mutex<Devices>>,
    seat: Mutex<Option<WlSeat>>,
    owned: Option<Connection>,
}

impl Drop for SerialsInner {
    fn drop(&mut self) {
        let Some(connection) = &self.owned else {
            return;
        };
        let mut devices = self.devices.lock();
        if let Some(pointer) = devices.pointer.take()
            && pointer.version() >= RELEASE_SINCE_VERSION
        {
            pointer.release();
        }
        if let Some(keyboard) = devices.keyboard.take()
            && keyboard.version() >= RELEASE_SINCE_VERSION
        {
            keyboard.release();
        }
        if let Some(seat) = self.seat.lock().take()
            && seat.version() >= crate::display::SEAT_RELEASE_SINCE_VERSION
        {
            seat.release();
        }
        if let Err(error) = connection.flush() {
            tracing::debug!(%error, "failed to flush input device release");
        }
    }
}

#[derive(Default)]
struct Devices {
    pointer: Option<WlPointer>,
    keyboard: Option<WlKeyboard>,
}

#[derive(Default)]
struct SerialLog {
    slots: [AtomicU64; SerialKind::ALL.len()],
    order: AtomicU32,
    waiting: Mutex<Vec<(SerialKind, Waker)>>,
}

impl SerialLog {
    fn record(&self, kind: SerialKind, serial: u32) {
        let order = self
            .order
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1)
            .max(1);
        self.slots[kind as usize].store(
            u64::from(order) << 32 | u64::from(serial),
            Ordering::Release,
        );
        let woken: Vec<Waker> = {
            let mut waiting = self.waiting.lock();
            let (woken, remaining) = waiting
                .drain(..)
                .partition(|(waiter_kind, _)| *waiter_kind == kind);
            *waiting = remaining;
            woken.into_iter().map(|(_, waker)| waker).collect()
        };
        woken.into_iter().for_each(Waker::wake);
    }

    fn clear(&self) {
        for slot in &self.slots {
            slot.store(0, Ordering::Release);
        }
    }

    fn latest(&self, among: &[SerialKind]) -> Option<Serial> {
        among
            .iter()
            .filter_map(|kind| {
                let packed = self.slots[*kind as usize].load(Ordering::Acquire);
                let order = (packed >> 32) as u32;
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "the low half of a slot is the serial"
                )]
                let serial = packed as u32;
                (order != 0).then_some((
                    order,
                    Serial {
                        serial,
                        kind: *kind,
                    },
                ))
            })
            .max_by_key(|(order, _)| *order)
            .map(|(_, latest)| latest)
    }
}

struct SeatListener {
    log: Arc<SerialLog>,
    devices: Arc<Mutex<Devices>>,
}

impl SeatListener {
    fn event(&self, seat: &WlSeat, event: wl_seat::Event) {
        let wl_seat::Event::Capabilities { capabilities } = event else {
            return;
        };
        let mut devices = self.devices.lock();
        if capabilities.contains(Capability::POINTER) && devices.pointer.is_none() {
            let log = Arc::clone(&self.log);
            let route = Route::tap::<WlPointer>(move |_, event| record_pointer(&log, &event));
            devices.pointer = Some(seat.via(&route).get_pointer());
        }
        if capabilities.contains(Capability::KEYBOARD) && devices.keyboard.is_none() {
            let log = Arc::clone(&self.log);
            let route = Route::tap::<WlKeyboard>(move |_, event| record_keyboard(&log, &event));
            devices.keyboard = Some(seat.via(&route).get_keyboard());
        }
        drop(devices);
        if let Err(error) = rikka_wayland_client::Proxy::handle(seat)
            .connection()
            .flush()
        {
            tracing::debug!(%error, "failed to flush input device requests");
        }
    }
}

fn record_pointer(log: &SerialLog, event: &wl_pointer::Event) {
    let recorded = match event {
        wl_pointer::Event::Enter { serial, .. } => Some((SerialKind::PointerEntered, *serial)),
        wl_pointer::Event::Leave { serial, .. } => Some((SerialKind::PointerLeft, *serial)),
        wl_pointer::Event::Button {
            serial,
            state: Ok(ButtonState::Pressed),
            ..
        } => Some((SerialKind::ButtonPressed, *serial)),
        wl_pointer::Event::Button {
            serial,
            state: Ok(ButtonState::Released),
            ..
        } => Some((SerialKind::ButtonReleased, *serial)),
        _ => None,
    };
    if let Some((kind, serial)) = recorded {
        log.record(kind, serial);
    }
}

fn record_keyboard(log: &SerialLog, event: &wl_keyboard::Event) {
    let recorded = match event {
        wl_keyboard::Event::Enter { serial, .. } => Some((SerialKind::KeyboardEntered, *serial)),
        wl_keyboard::Event::Leave { serial, .. } => Some((SerialKind::KeyboardLeft, *serial)),
        wl_keyboard::Event::Key {
            serial,
            state: Ok(KeyState::Pressed),
            ..
        } => Some((SerialKind::KeyPressed, *serial)),
        wl_keyboard::Event::Key {
            serial,
            state: Ok(KeyState::Released),
            ..
        } => Some((SerialKind::KeyReleased, *serial)),
        wl_keyboard::Event::Modifiers { serial, .. } => {
            Some((SerialKind::ModifiersChanged, *serial))
        }
        _ => None,
    };
    if let Some((kind, serial)) = recorded {
        log.record(kind, serial);
    }
}
