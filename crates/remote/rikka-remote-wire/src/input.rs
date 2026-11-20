use crate::bytes::{Reader, Writer};
use crate::error::DecodeError;
use crate::types::Button;

const KEY: u8 = 0x01;
const BUTTON: u8 = 0x02;
const WHEEL: u8 = 0x03;
const SCROLL: u8 = 0x04;
const RELATIVE: u8 = 0x05;
const TEXT: u8 = 0x06;
const RELEASE_ALL: u8 = 0x07;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Position {
    pub x: u16,
    pub y: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    Key {
        code: u16,
        pressed: bool,
    },
    Button {
        button: Button,
        pressed: bool,
        at: Position,
    },
    Wheel {
        dx: i32,
        dy: i32,
    },
    Scroll {
        dx: f32,
        dy: f32,
    },
    Relative {
        dx: f32,
        dy: f32,
    },
    Text(String),
    ReleaseAll,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Parsed<T> {
    Complete { value: T, consumed: usize },
    Incomplete,
}

impl InputEvent {
    pub fn encode(&self, out: &mut Vec<u8>) {
        let mut writer = Writer(out);
        match self {
            InputEvent::Key { code, pressed } => {
                writer.u8(KEY).u16(*code).bool(*pressed);
            }
            InputEvent::Button {
                button,
                pressed,
                at,
            } => {
                writer
                    .u8(BUTTON)
                    .u8(button.to_wire())
                    .bool(*pressed)
                    .u16(at.x)
                    .u16(at.y);
            }
            InputEvent::Wheel { dx, dy } => {
                writer.u8(WHEEL).i32(*dx).i32(*dy);
            }
            InputEvent::Scroll { dx, dy } => {
                writer.u8(SCROLL).f32(*dx).f32(*dy);
            }
            InputEvent::Relative { dx, dy } => {
                writer.u8(RELATIVE).f32(*dx).f32(*dy);
            }
            InputEvent::Text(text) => {
                writer.u8(TEXT).str(text);
            }
            InputEvent::ReleaseAll => {
                writer.u8(RELEASE_ALL);
            }
        }
    }

    pub fn parse(bytes: &[u8]) -> Result<Parsed<InputEvent>, DecodeError> {
        let Some((kind, body)) = bytes.split_first() else {
            return Ok(Parsed::Incomplete);
        };
        let mut reader = Reader::new(*kind, body);
        let event = match *kind {
            KEY => read_key(&mut reader),
            BUTTON => read_button(&mut reader),
            WHEEL => read_wheel(&mut reader),
            SCROLL => read_floats(&mut reader).map(|(dx, dy)| InputEvent::Scroll { dx, dy }),
            RELATIVE => read_floats(&mut reader).map(|(dx, dy)| InputEvent::Relative { dx, dy }),
            TEXT => reader.str().map(|text| InputEvent::Text(text.to_owned())),
            RELEASE_ALL => Ok(InputEvent::ReleaseAll),
            other => return Err(DecodeError::UnknownType(other)),
        };
        complete(event, 1 + reader.consumed_from(body))
    }
}

fn read_key(reader: &mut Reader<'_>) -> Result<InputEvent, DecodeError> {
    Ok(InputEvent::Key {
        code: reader.u16()?,
        pressed: reader.bool()?,
    })
}

fn read_button(reader: &mut Reader<'_>) -> Result<InputEvent, DecodeError> {
    Ok(InputEvent::Button {
        button: Button::from_wire(reader.u8()?)?,
        pressed: reader.bool()?,
        at: Position {
            x: reader.u16()?,
            y: reader.u16()?,
        },
    })
}

fn read_wheel(reader: &mut Reader<'_>) -> Result<InputEvent, DecodeError> {
    Ok(InputEvent::Wheel {
        dx: reader.i32()?,
        dy: reader.i32()?,
    })
}

fn read_floats(reader: &mut Reader<'_>) -> Result<(f32, f32), DecodeError> {
    Ok((reader.f32()?, reader.f32()?))
}

fn complete<T>(read: Result<T, DecodeError>, consumed: usize) -> Result<Parsed<T>, DecodeError> {
    match read {
        Ok(value) => Ok(Parsed::Complete { value, consumed }),
        Err(DecodeError::Truncated(_)) => Ok(Parsed::Incomplete),
        Err(error) => Err(error),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileHeader {
    pub id: u32,
    pub name: String,
    pub size: u64,
}

impl FileHeader {
    pub fn encode(&self, out: &mut Vec<u8>) {
        Writer(out).u32(self.id).str(&self.name).u64(self.size);
    }

    pub fn parse(bytes: &[u8]) -> Result<Parsed<FileHeader>, DecodeError> {
        let mut reader = Reader::new(0, bytes);
        let header = (|| {
            Ok(FileHeader {
                id: reader.u32()?,
                name: reader.str()?.to_owned(),
                size: reader.u64()?,
            })
        })();
        let consumed = reader.consumed_from(bytes);
        complete(header, consumed)
    }
}
