use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use rikka_power::Power;
use rikka_seam::{Request, SeamClient};
use rikka_session::lock::{self as session_lock, LockState};
use rikka_session::shortcuts::Shortcut;
use rikka_settings::session::lock::{
    BEFORE_SLEEP, BEFORE_SLEEP_DEFAULT, DOMAIN, IDLE, IDLE_DEFAULT, schema,
};
use shell_kit::{
    Feature, Sealed, SessionLock, Shell, SurfaceDesc, appearance, detached, idle_minutes,
};

use crate::gate::Gate;
use crate::guard::{Guard, Sources};
use crate::service::{BeforeSleep, CAPABILITY, LOCK, LockService};
use crate::sleep::SleepInhibitor;
use crate::view;

const CHECKING: &str = "rikka-lock-check";

#[derive(Debug)]
pub struct ScreenLock {
    power: Power,
    lock: SessionLock,
}

impl ScreenLock {
    #[must_use]
    pub fn new(power: Power, lock: SessionLock) -> ScreenLock {
        ScreenLock { power, lock }
    }
}

impl Feature for ScreenLock {
    fn install(self, shell: Shell) -> Shell {
        let shell = shell.settings(schema());
        let seam = shell.seam();
        let lock = self.lock;
        let gate = Gate::new(lock.clone());
        let sleep_inhibitor = SleepInhibitor::watch(seam.clone());
        let wallpaper: Rc<RefCell<Option<PathBuf>>> = Rc::default();

        lock.on_change({
            let (gate, sleep_inhibitor) = (gate.clone(), sleep_inhibitor.clone());
            move |sealed| match sealed {
                Sealed::Locking => gate.begin(),
                Sealed::Locked => {
                    sleep_inhibitor.release();
                    gate.begin();
                }
                Sealed::Open => {
                    sleep_inhibitor.release();
                    gate.end();
                }
            }
        });

        let idle = shell.idle();
        idle.after(idle_minutes(IDLE_DEFAULT));
        idle.on_idle(|host| host.seam().notify(Request::new(CAPABILITY, LOCK)));

        guard_if_unguarded(seam.clone());

        let sources = {
            let (gate, wall, keyboard, power, wallpaper) = (
                gate.clone(),
                shell.wall(),
                shell.keyboard(),
                self.power,
                Rc::clone(&wallpaper),
            );
            move || Sources {
                gate: gate.clone(),
                wall: wall.clone(),
                keyboard: keyboard.clone(),
                power: power.clone(),
                wallpaper: wallpaper.borrow().clone(),
            }
        };

        let (shell, before_sleep) = shell.hold(BeforeSleep {
            locks: BEFORE_SLEEP_DEFAULT,
        });
        shell
            .service(LockService::new(gate, sleep_inhibitor, before_sleep))
            .on_shortcut(Shortcut::LockScreen, Request::new(CAPABILITY, LOCK))
            .watch(session_lock::CAPABILITY, |host, incoming| {
                let needs_locker = incoming.event().is_some_and(|event| {
                    session_lock::is_lock_request(event)
                        || session_lock::changed(event).is_some_and(LockState::needs_locker)
                });
                if needs_locker {
                    host.seam().notify(Request::new(CAPABILITY, LOCK));
                }
            })
            .follow(DOMAIN, IDLE, move |_, chosen: i64| {
                idle.after(idle_minutes(chosen))
            })
            .follow(DOMAIN, BEFORE_SLEEP, move |host, locks: bool| {
                host.get_mut(before_sleep).locks = locks;
            })
            .follow(
                appearance::DOMAIN,
                appearance::WALLPAPER,
                move |_, value: String| {
                    *wallpaper.borrow_mut() = shell_wallpaper::file_of_setting(value)
                },
            )
            .surface(
                SurfaceDesc::per_output(move |screen| Guard::new(screen, sources()), view::view)
                    .on_start(Guard::start)
                    .session_lock(&lock),
            )
    }
}

fn guard_if_unguarded(seam: SeamClient) {
    detached(CHECKING, move || {
        let state = session_lock::state(&seam)
            .blocking_wait()
            .and_then(|payload| payload.decode::<LockState>());
        match state {
            Ok(LockState::Sealed) => {
                tracing::info!("session locked");
                seam.notify(Request::new(CAPABILITY, LOCK));
            }
            Ok(state) if state.needs_locker() => {
                tracing::warn!("session locked without a lock surface");
                seam.notify(Request::new(CAPABILITY, LOCK));
            }
            Ok(_) => {}
            Err(error) => tracing::debug!("failed to query session lock state: {error}"),
        }
    });
}
