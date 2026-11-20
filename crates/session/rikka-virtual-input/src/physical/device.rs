use std::ops::ControlFlow;
use std::os::fd::AsFd as _;

use rikka_wayland::{Clock, EventLoop, Host};
use rikka_wayland_client::wl_keyboard::{self, KeyState, KeymapFormat, WlKeyboard};
use rikka_wayland_client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
use rikka_wayland_client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;
use rikka_worker::CommandReceiver;

use super::Command;
use crate::error::{Error, Result};
use crate::seat::SeatHandle;

const KEYMAP_FORMAT_XKB_V1: u32 = 1;

pub(super) struct Device {
    host: Host,
    events: EventLoop<Board>,
    board: Board,
}

struct Board {
    keyboard: WlKeyboard,
    board: ZwpVirtualKeyboardV1,
    keymapped: bool,
    clock: Clock,
    pressed: Vec<u32>,
}

impl Device {
    pub(super) fn open(seat: &SeatHandle) -> Result<Device> {
        let display = seat.host.display();
        let manager: ZwpVirtualKeyboardManagerV1 = display.bind(1..=1)?;
        let mut events = display.event_loop::<Board>();
        events.on::<WlKeyboard>(on_wl_keyboard);
        let keyboard = seat.seat.via(&events.route()).get_keyboard();
        let mut board = Board {
            keyboard,
            board: manager.create_virtual_keyboard(&seat.seat),
            keymapped: false,
            clock: Clock::new(),
            pressed: Vec::new(),
        };
        events.settle(&mut board)?;
        if !board.keymapped {
            board.close(&seat.host);
            return Err(Error::NoSeatKeymap);
        }
        Ok(Device {
            host: seat.host.clone(),
            events,
            board,
        })
    }

    pub(super) fn run(&mut self, commands: &CommandReceiver<Command>) {
        let served = self
            .events
            .run(&mut self.board, commands, |board, command| {
                match command {
                    Command::Press(code) => board.press(code),
                    Command::Release(code) => board.release(code),
                    Command::ReleaseAll => board.release_all(),
                    Command::Stop => return ControlFlow::Break(()),
                }
                ControlFlow::Continue(())
            });
        if let Err(error) = served {
            tracing::warn!(%error, "virtual keyboard connection failed");
        }
        self.board.close(&self.host);
    }
}

impl Board {
    fn press(&mut self, code: u32) {
        if self.pressed.contains(&code) {
            return;
        }
        self.pressed.push(code);
        self.send(code, KeyState::Pressed);
    }

    fn release(&mut self, code: u32) {
        let Some(index) = self.pressed.iter().position(|pressed| *pressed == code) else {
            return;
        };
        self.pressed.remove(index);
        self.send(code, KeyState::Released);
    }

    fn release_all(&mut self) {
        for code in std::mem::take(&mut self.pressed).into_iter().rev() {
            self.send(code, KeyState::Released);
        }
    }

    fn send(&mut self, code: u32, state: KeyState) {
        let time = self.clock.now();
        self.board
            .send_key(time, code, rikka_wayland_client::raw_value(Ok(state)));
    }

    fn close(&mut self, host: &Host) {
        self.release_all();
        self.board.destroy();
        self.keyboard.release();
        if let Err(error) = host.display().flush() {
            tracing::debug!(%error, "failed to flush virtual keyboard teardown");
        }
    }
}

fn on_wl_keyboard(board: &mut Board, _: WlKeyboard, event: wl_keyboard::Event) {
    if let wl_keyboard::Event::Keymap {
        format: Ok(KeymapFormat::XkbV1),
        fd,
        size,
    } = event
    {
        board.board.keymap(KEYMAP_FORMAT_XKB_V1, fd.as_fd(), size);
        board.keymapped = true;
    }
}
