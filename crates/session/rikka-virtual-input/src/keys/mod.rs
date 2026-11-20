mod device;
mod queue;

use std::time::Duration;

use rikka_input::Chord;

use crate::cap::Cap;
use crate::error::{Error, Result, from_worker};
use crate::keymap;
use crate::modifier_state::ModifierState;
use crate::seat::SeatHandle;

use device::Device;
use queue::{Command, Queue};

const TAP_HOLD: Duration = Duration::from_millis(8);

type Worker = rikka_worker::Worker<Queue>;

fn spawn(seat: SeatHandle) -> Result<Worker> {
    let (queue, commands) = queue::channel();

    Worker::spawn("rikka-keyboard", queue, move || {
        let mut device = Device::open(&seat)?;
        Ok(move || device.run(&commands))
    })
    .map_err(from_worker)
}

#[derive(Debug)]
pub struct TextKeyboard {
    worker: Worker,
}

impl TextKeyboard {
    pub(crate) fn new(seat: SeatHandle) -> Result<TextKeyboard> {
        Ok(TextKeyboard {
            worker: spawn(seat)?,
        })
    }

    pub fn press(&self, chord: Chord) -> Result<()> {
        self.push_press(chord, TAP_HOLD)
    }

    pub fn hold_for(&self, chord: Chord, hold: Duration) -> Result<()> {
        self.push_press(chord, hold)
    }

    fn push_press(&self, chord: Chord, hold: Duration) -> Result<()> {
        keymap::for_chord(chord)?;
        self.worker.handle().push(Command::Press { chord, hold })
    }

    pub fn set_modifiers(&self, state: ModifierState) -> Result<()> {
        let queue = self.worker.handle();
        queue.push(Command::Modifiers {
            depressed: state.depressed_mask(),
            locked: state.locked_mask(),
        })?;
        queue.sync()
    }

    pub fn type_text(&self, text: &str) -> Result<()> {
        for segment in keymap::for_text(text) {
            self.worker.handle().push(Command::Text {
                keymap: segment.keymap,
                codes: segment.codes,
            })?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Keyboard {
    worker: Worker,
    caps: Vec<Cap>,
}

impl Keyboard {
    pub(crate) fn new(seat: SeatHandle, caps: impl IntoIterator<Item = Cap>) -> Result<Keyboard> {
        let mut unique: Vec<Cap> = Vec::new();
        for cap in caps {
            let code = keymap::cap_code(cap)?;
            if !unique.iter().any(|known| known.code() == Some(code)) {
                unique.push(cap);
            }
        }
        let keymap = keymap::for_caps(&unique)?;

        let worker = spawn(seat)?;
        worker.handle().push(Command::Keymap(keymap))?;
        Ok(Keyboard {
            worker,
            caps: unique,
        })
    }

    pub fn set_modifiers(&self, state: ModifierState) -> Result<()> {
        self.worker.handle().push(Command::Modifiers {
            depressed: state.depressed_mask(),
            locked: state.locked_mask(),
        })
    }

    pub fn press(&self, cap: Cap) -> Result<()> {
        self.push_key(cap, true)
    }

    pub fn release(&self, cap: Cap) -> Result<()> {
        self.push_key(cap, false)
    }

    fn push_key(&self, cap: Cap, pressed: bool) -> Result<()> {
        if !self.caps.contains(&cap) {
            return Err(Error::Unsupported(cap.key()));
        }
        self.worker.handle().push(Command::Key {
            code: keymap::cap_code(cap)?,
            pressed,
        })
    }
}
