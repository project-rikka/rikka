use std::sync::Arc;

use rikka_remote_wire::{Button, InputEvent, POSITION_EXTENT, Position};
use rikka_types::{KeyCode, PointerButton};
use rikka_virtual_input::{AbsolutePosition, PhysicalKeyboard, Pointer, Seat, TextKeyboard};
use rikka_worker::{CommandReceiver, CommandSender, Worker, command_channel};

use crate::media::{DisplayInfo, MediaError};

const THREAD: &str = "rikka-remote-input";

pub(crate) enum InputCommand {
    Event(InputEvent),
    Move(Position),
    Show(DisplayInfo),
    Stop,
}

pub(crate) struct InputWorker {
    worker: Worker<Arc<CommandSender<InputCommand>>>,
}

impl InputWorker {
    pub(crate) fn start() -> Result<InputWorker, MediaError> {
        let (commands, received) =
            command_channel(|| InputCommand::Stop).map_err(|error| MediaError::Thread {
                thread: THREAD,
                error,
            })?;
        let worker = Worker::spawn(THREAD, Arc::new(commands), move || {
            let injector = Injector::connect().map_err(MediaError::Input)?;
            Ok(move || serve(injector, &received))
        })
        .map_err(|error| MediaError::worker(THREAD, error))?;
        Ok(InputWorker { worker })
    }

    pub(crate) fn handle(&self) -> InputHandle {
        InputHandle(Arc::clone(self.worker.handle()))
    }
}

fn serve(mut injector: Injector, commands: &CommandReceiver<InputCommand>) {
    loop {
        for command in commands.drain() {
            match command {
                InputCommand::Event(event) => injector.apply(event),
                InputCommand::Move(at) => injector.move_to(at),
                InputCommand::Show(display) => injector.show(&display),
                InputCommand::Stop => return,
            }
        }
        if let Err(error) = commands.wait() {
            tracing::warn!("input commands stopped: {error}");
            return;
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct InputHandle(Arc<CommandSender<InputCommand>>);

impl InputHandle {
    pub(crate) fn send(&self, command: InputCommand) {
        if let Err(error) = self.0.send(command) {
            tracing::debug!("the input thread is gone: {error}");
        }
    }
}

struct Injector {
    seat: Seat,
    keyboard: PhysicalKeyboard,
    text: TextKeyboard,
    pointer: Pointer,
    output: Option<String>,
    held: Vec<PointerButton>,
}

impl Injector {
    pub(crate) fn connect() -> rikka_virtual_input::Result<Injector> {
        let seat = Seat::connect()?;
        Ok(Injector {
            keyboard: seat.physical_keyboard()?,
            text: seat.text_keyboard()?,
            pointer: seat.pointer()?,
            seat,
            output: None,
            held: Vec::new(),
        })
    }

    pub(crate) fn show(&mut self, display: &DisplayInfo) {
        if self.output.as_deref() == Some(display.name.as_str()) {
            return;
        }
        self.release_buttons();
        match self.seat.pointer_on_output(&display.name) {
            Ok(pointer) => {
                self.pointer = pointer;
                self.output = Some(display.name.clone());
            }
            Err(error) => {
                let output = display.name.as_str();
                tracing::warn!(output, "cannot point at the display: {error}");
            }
        }
    }

    pub(crate) fn move_to(&self, at: Position) {
        let position = AbsolutePosition::new(
            u32::from(at.x),
            u32::from(at.y),
            POSITION_EXTENT,
            POSITION_EXTENT,
        );
        report(self.pointer.move_to(position));
    }

    pub(crate) fn apply(&mut self, input: InputEvent) {
        match input {
            InputEvent::Relative { dx, dy } => {
                report(self.pointer.move_by(f64::from(dx), f64::from(dy)))
            }
            InputEvent::Button {
                button,
                pressed,
                at,
            } => {
                self.move_to(at);
                self.button(pointer_button(button), pressed);
            }
            InputEvent::Wheel { dx, dy } => report(self.pointer.scroll_value120(dx, dy)),
            InputEvent::Scroll { dx, dy } => {
                report(self.pointer.scroll_continuous(f64::from(dx), f64::from(dy)))
            }
            InputEvent::Key { code, pressed } => {
                let code = KeyCode::from_evdev(u32::from(code));
                report(match pressed {
                    true => self.keyboard.press(code),
                    false => self.keyboard.release(code),
                });
            }
            InputEvent::Text(text) => report(self.text.type_text(&text)),
            InputEvent::ReleaseAll => self.release_all(),
        }
    }

    pub(crate) fn release_all(&mut self) {
        report(self.keyboard.release_all());
        self.release_buttons();
    }

    fn button(&mut self, button: PointerButton, pressed: bool) {
        let held = self.held.contains(&button);
        match (pressed, held) {
            (true, false) => {
                self.held.push(button);
                report(self.pointer.press(button));
            }
            (false, true) => {
                self.held.retain(|other| *other != button);
                report(self.pointer.release(button));
            }
            _ => {}
        }
    }

    fn release_buttons(&mut self) {
        for button in std::mem::take(&mut self.held) {
            report(self.pointer.release(button));
        }
    }
}

impl Drop for Injector {
    fn drop(&mut self) {
        self.release_all();
    }
}

fn pointer_button(button: Button) -> PointerButton {
    match button {
        Button::Primary => PointerButton::Primary,
        Button::Middle => PointerButton::Middle,
        Button::Secondary => PointerButton::Secondary,
        Button::Back => PointerButton::Back,
        Button::Forward => PointerButton::Forward,
    }
}

fn report(result: rikka_virtual_input::Result<()>) {
    if let Err(error) = result {
        tracing::debug!("cannot inject input: {error}");
    }
}
