use std::path::Path;
use std::time::Duration;

use rikka_input::Chord;
use rikka_types::{Modifiers, PointerButton};
use rikka_virtual_input::{AbsolutePosition, Pointer, Seat, TextKeyboard};

use crate::backend::Point;
use crate::error::{Error, Result, input};

const CLICK_HOLD: Duration = Duration::from_millis(40);

const MULTI_CLICK_GAP: Duration = Duration::from_millis(90);

const DRAG_STEPS: u32 = 12;
const DRAG_STEP_INTERVAL: Duration = Duration::from_millis(16);

#[derive(Debug)]
pub(crate) struct InputDriver {
    pointer: Pointer,
    keyboard: TextKeyboard,
    size: Point,
    position: Point,
}

impl InputDriver {
    pub(crate) fn connect(socket: &Path, size: Point) -> Result<InputDriver> {
        InputDriver::new(&Seat::connect_to(socket).map_err(input)?, size)
    }

    pub(crate) fn for_seat(seat: &str, size: Point) -> Result<InputDriver> {
        InputDriver::new(&Seat::named(seat).map_err(input)?, size)
    }

    fn new(seat: &Seat, size: Point) -> Result<InputDriver> {
        let mut driver = InputDriver {
            pointer: seat.pointer().map_err(input)?,
            keyboard: seat.text_keyboard().map_err(input)?,
            size,
            position: (0, 0),
        };
        driver.move_to(size.0 / 2, size.1 / 2)?;
        Ok(driver)
    }

    pub(crate) fn resize(&mut self, size: Point) {
        self.size = size;
        self.position = (self.position.0.min(size.0), self.position.1.min(size.1));
    }

    pub(crate) fn position(&self) -> Point {
        self.position
    }

    pub(crate) fn move_to(&mut self, x: u32, y: u32) -> Result<()> {
        let position = self.absolute_position((x, y))?;
        self.pointer.move_to(position).map_err(input)?;
        self.position = (x, y);
        Ok(())
    }

    pub(crate) fn click(
        &mut self,
        button: PointerButton,
        count: u32,
        modifiers: Modifiers,
        at: Option<Point>,
    ) -> Result<()> {
        self.move_if_given(at)?;
        self.with_modifiers(modifiers, |driver| {
            for nth in 0..count.max(1) {
                if nth > 0 {
                    std::thread::sleep(MULTI_CLICK_GAP);
                }
                driver.pointer.press(button).map_err(input)?;
                std::thread::sleep(CLICK_HOLD);
                driver.pointer.release(button).map_err(input)?;
            }
            Ok(())
        })
    }

    pub(crate) fn press(&mut self, button: PointerButton, at: Option<Point>) -> Result<()> {
        self.move_if_given(at)?;
        self.pointer.press(button).map_err(input)
    }

    pub(crate) fn release(&mut self, button: PointerButton, at: Option<Point>) -> Result<()> {
        let moved = self.move_if_given(at);
        let released = self.pointer.release(button).map_err(input);
        moved.and(released)
    }

    pub(crate) fn drag(
        &mut self,
        button: PointerButton,
        to: Point,
        modifiers: Modifiers,
    ) -> Result<()> {
        self.absolute_position(to)?;
        self.with_modifiers(modifiers, |driver| {
            let from = driver.position;
            driver.pointer.press(button).map_err(input)?;
            let moved = (1..=DRAG_STEPS).try_for_each(|step| {
                std::thread::sleep(DRAG_STEP_INTERVAL);
                let t = f64::from(step) / f64::from(DRAG_STEPS);
                driver.move_to(lerp(from.0, to.0, t), lerp(from.1, to.1, t))
            });

            std::thread::sleep(DRAG_STEP_INTERVAL);
            let released = driver.pointer.release(button).map_err(input);
            moved.and(released)
        })
    }

    pub(crate) fn scroll(
        &mut self,
        dx: f64,
        dy: f64,
        modifiers: Modifiers,
        at: Option<Point>,
    ) -> Result<()> {
        self.move_if_given(at)?;
        self.with_modifiers(modifiers, |driver| {
            driver.pointer.scroll(dx, dy).map_err(input)
        })
    }

    pub(crate) fn hold_modifiers(&self, modifiers: Modifiers) -> Result<()> {
        self.keyboard.set_modifiers(modifiers.into()).map_err(input)
    }

    pub(crate) fn key(&self, chord: Chord) -> Result<()> {
        self.keyboard.press(chord).map_err(input)
    }

    pub(crate) fn hold_key(&self, chord: Chord, duration: Duration) -> Result<()> {
        self.keyboard.hold_for(chord, duration).map_err(input)
    }

    pub(crate) fn type_text(&self, text: &str) -> Result<()> {
        self.keyboard.type_text(text).map_err(input)
    }

    fn move_if_given(&mut self, at: Option<Point>) -> Result<()> {
        match at {
            Some((x, y)) => self.move_to(x, y),
            None => Ok(()),
        }
    }

    fn absolute_position(&self, (x, y): Point) -> Result<AbsolutePosition> {
        let position = AbsolutePosition::new(x, y, self.size.0, self.size.1);
        if !position.is_in_bounds() {
            return Err(Error::OutOfBounds {
                x,
                y,
                width: self.size.0,
                height: self.size.1,
            });
        }
        Ok(position)
    }

    fn with_modifiers(
        &mut self,
        modifiers: Modifiers,
        action: impl FnOnce(&mut InputDriver) -> Result<()>,
    ) -> Result<()> {
        if modifiers == Modifiers::NONE {
            return action(self);
        }

        self.keyboard
            .set_modifiers(modifiers.into())
            .map_err(input)?;
        let acted = action(self);
        let flushed = self.pointer.sync().map_err(input);
        let released = self
            .keyboard
            .set_modifiers(Modifiers::NONE.into())
            .map_err(input);
        acted.and(flushed).and(released)
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "interpolates between two u32 coordinates with t in [0, 1]"
)]
fn lerp(from: u32, to: u32, t: f64) -> u32 {
    (f64::from(from) + (f64::from(to) - f64::from(from)) * t)
        .round()
        .max(0.0) as u32
}
