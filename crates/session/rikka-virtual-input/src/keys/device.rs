use std::fs::File;
use std::io::Write as _;
use std::os::fd::AsFd as _;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use rikka_input::Chord;
use rikka_wayland::{Clock, EventLoop, Host};
use rikka_wayland_client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
use rikka_wayland_client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;
use rustix::fs::{MemfdFlags, memfd_create};

use crate::error::{Error, Result};
use crate::keymap;
use crate::keys::queue::Command;
use crate::seat::SeatHandle;

const KEYMAP_FORMAT_XKB_V1: u32 = 1;
const KEYMAP_NAME: &str = "rikka-keymap";

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct ModifierMasks {
    depressed: u32,
    locked: u32,
}

pub(super) struct Device {
    host: Host,
    events: EventLoop<()>,
    keyboard: ZwpVirtualKeyboardV1,
    keymap: Option<String>,
    clock: Clock,
    pressed: Vec<u32>,
    requested: ModifierMasks,
    sent: ModifierMasks,
}

impl Device {
    pub(super) fn open(seat: &SeatHandle) -> Result<Device> {
        let display = seat.host.display();
        let manager: ZwpVirtualKeyboardManagerV1 = display.bind(1..=1)?;
        let keyboard = manager.create_virtual_keyboard(&seat.seat);
        Ok(Device {
            host: seat.host.clone(),
            events: display.event_loop(),
            keyboard,
            keymap: None,
            clock: Clock::new(),
            pressed: Vec::new(),
            requested: ModifierMasks::default(),
            sent: ModifierMasks::default(),
        })
    }

    pub(super) fn run(&mut self, commands: &Receiver<Command>) {
        while let Ok(command) = commands.recv() {
            let result = match command {
                Command::Press { chord, hold } => self.press(chord, hold),
                Command::Keymap(keymap) => self.upload(keymap).and_then(|()| self.settle()),
                Command::Modifiers { depressed, locked } => {
                    self.set_modifiers(ModifierMasks { depressed, locked })
                }
                Command::Key { code, pressed } => self.key(code, pressed),
                Command::Text { keymap, codes } => self.type_codes(&keymap, &codes),
                Command::Sync(done) => {
                    if done.send(()).is_err() {
                        tracing::trace!("keyboard sync waiter is gone");
                    }
                    Ok(())
                }
                Command::Stop => break,
            };
            if let Err(error) = result {
                tracing::warn!(%error, "virtual keyboard request failed");
            }
        }
        self.release_all();
        self.keyboard.destroy();
        if let Err(error) = self.host.display().flush() {
            tracing::debug!(%error, "failed to flush virtual keyboard teardown");
        }
    }

    fn set_modifiers(&mut self, requested: ModifierMasks) -> Result<()> {
        self.requested = requested;
        self.send_modifiers()?;
        self.flush()
    }

    fn key(&mut self, code: u32, pressed: bool) -> Result<()> {
        self.pressed.retain(|pressed_code| *pressed_code != code);
        if pressed {
            self.pressed.push(code);
        }
        let time = self.clock.now();
        self.keyboard.send_key(time, code, u32::from(pressed));
        self.send_modifiers()?;
        self.flush()
    }

    fn send_modifiers(&mut self) -> Result<()> {
        let depressed = self
            .pressed
            .iter()
            .filter_map(|code| keymap::modifier_bit(*code))
            .fold(self.requested.depressed, |mask, bit| mask | bit);
        let state = ModifierMasks {
            depressed,
            locked: self.requested.locked,
        };
        if self.sent == state {
            return Ok(());
        }
        if self.keymap.is_none() {
            self.upload(keymap::empty())?;
            self.settle()?;
        }
        self.sent = state;
        self.keyboard.modifiers(state.depressed, 0, state.locked, 0);
        Ok(())
    }

    fn release_all(&mut self) {
        if self.keymap.is_none() {
            return;
        }

        for code in std::mem::take(&mut self.pressed) {
            let time = self.clock.now();
            self.keyboard.send_key(time, code, 0);
        }
        self.requested = ModifierMasks::default();
        self.sent = ModifierMasks::default();
        self.keyboard.modifiers(0, 0, 0, 0);
        if let Err(error) = self.settle() {
            tracing::debug!(%error, "failed to release virtual keyboard keys");
        }
    }

    fn type_codes(&mut self, keymap: &str, codes: &[u32]) -> Result<()> {
        self.upload(keymap.to_owned())?;
        self.settle()?;
        for code in codes {
            self.key(*code, true)?;
            self.key(*code, false)?;
        }
        self.settle()
    }

    fn press(&mut self, chord: Chord, hold: Duration) -> Result<()> {
        let (keymap, code) = keymap::for_chord(chord)?;
        self.upload(keymap)?;
        let modifiers: Vec<u32> = keymap::modifier_codes(chord.modifiers())
            .into_iter()
            .filter(|modifier| *modifier != code)
            .collect();
        for modifier in &modifiers {
            self.key(*modifier, true)?;
        }
        self.key(code, true)?;
        self.settle()?;

        std::thread::sleep(hold);

        self.key(code, false)?;
        for modifier in modifiers.iter().rev() {
            self.key(*modifier, false)?;
        }
        self.settle()
    }

    fn flush(&mut self) -> Result<()> {
        self.host.display().flush()?;
        Ok(())
    }

    fn settle(&mut self) -> Result<()> {
        self.events.settle(&mut ())?;
        Ok(())
    }

    fn upload(&mut self, keymap: String) -> Result<()> {
        if self.keymap.as_deref() == Some(keymap.as_str()) {
            return Ok(());
        }
        let size = u32::try_from(keymap.len() + 1).map_err(|_| Error::KeymapTooLarge)?;
        let memory =
            memfd_create(KEYMAP_NAME, MemfdFlags::CLOEXEC).map_err(std::io::Error::from)?;
        let mut file = File::from(memory);
        file.write_all(keymap.as_bytes())?;
        file.write_all(&[0])?;
        file.flush()?;
        self.keyboard
            .keymap(KEYMAP_FORMAT_XKB_V1, file.as_fd(), size);
        self.keymap = Some(keymap);
        Ok(())
    }
}
