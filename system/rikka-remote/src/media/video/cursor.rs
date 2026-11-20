use std::collections::VecDeque;

use rikka_capture::{
    CaptureBuffer, CaptureEvent, CursorFrames, CursorPosition, PixelFormat, Screen, Source,
};
use rikka_remote_wire::{CURSOR_SHAPES, CursorShape, HIDDEN_CURSOR, ServerMessage};

use crate::media::MediaError;

const TRANSLUCENT: [PixelFormat; 2] = [PixelFormat::Argb8888, PixelFormat::Abgr8888];

#[derive(PartialEq, Eq)]
struct Shape {
    rgba: Vec<u8>,
    size: (u32, u32),
    hotspot: (i32, i32),
}

pub(crate) enum CursorNews {
    Message(ServerMessage),
    Pointer { x: i32, y: i32 },
}

pub(crate) struct CursorTrack {
    frames: CursorFrames,
    buffer: Option<CaptureBuffer>,
    generation: u64,
    shapes: VecDeque<(Shape, u32)>,
    next: u32,
    shown: u32,
    position: Option<CursorPosition>,
    pointed: Option<(i32, i32)>,
}

impl CursorTrack {
    pub(crate) fn open(screen: &mut Screen, source: &Source) -> Result<CursorTrack, MediaError> {
        let frames = screen.cursor_frames(source).map_err(MediaError::Cursor)?;
        Ok(CursorTrack {
            frames,
            buffer: None,
            generation: 0,
            shapes: VecDeque::with_capacity(CURSOR_SHAPES),
            next: HIDDEN_CURSOR + 1,
            shown: HIDDEN_CURSOR,
            position: None,
            pointed: None,
        })
    }

    pub(crate) fn pump(&mut self, screen: &mut Screen) {
        let constraints = self.frames.frames().constraints();
        if constraints.is_empty() {
            return;
        }
        if constraints.generation != self.generation || self.buffer.is_none() {
            let Some(format) = constraints.preferred_shm(&TRANSLUCENT) else {
                tracing::debug!("the compositor offers no translucent cursor format");
                return;
            };
            match screen.shm_buffer(format, constraints.width, constraints.height) {
                Ok(buffer) => {
                    self.buffer = Some(buffer);
                    self.generation = constraints.generation;
                }
                Err(error) => {
                    tracing::debug!("cannot allocate a cursor buffer: {error}");
                    return;
                }
            }
        }
        if !self.frames.frames().is_capturing()
            && let Some(buffer) = &self.buffer
            && let Err(error) = self.frames.frames().capture(buffer)
        {
            tracing::debug!("cannot capture the cursor: {error}");
        }
    }

    pub(crate) fn collect(&mut self, pointer_locked: bool, news: &mut Vec<CursorNews>) {
        let position = self.frames.cursor_position();
        let appeared = position.is_some() != self.position.is_some();
        self.position = position;
        let mut reshaped = false;
        for event in self.frames.frames().events() {
            if let CaptureEvent::Ready(_) = event {
                reshaped |= self.reshape(news);
            }
        }
        if appeared || reshaped {
            let serial = match self.position {
                Some(_) => self.shown,
                None => HIDDEN_CURSOR,
            };
            news.push(CursorNews::Message(ServerMessage::Cursor(serial)));
        }
        let pointed = position
            .filter(|_| pointer_locked)
            .map(|position| (position.x, position.y));
        if pointed != self.pointed {
            self.pointed = pointed;
            if let Some((x, y)) = pointed {
                news.push(CursorNews::Pointer { x, y });
            }
        }
    }

    fn reshape(&mut self, news: &mut Vec<CursorNews>) -> bool {
        let Some(buffer) = &self.buffer else {
            return false;
        };
        let Some(pixels) = buffer.pixels() else {
            return false;
        };
        let size = (buffer.width(), buffer.height());
        let rgba = to_rgba(
            pixels,
            buffer.stride() as usize,
            size,
            buffer.format().is_bgr(),
        );
        let hotspot = self
            .position
            .map_or((0, 0), |position| (position.hotspot_x, position.hotspot_y));
        let key = Shape {
            rgba,
            size,
            hotspot,
        };
        if let Some(index) = self.shapes.iter().position(|(shape, _)| *shape == key)
            && let Some(known) = self.shapes.remove(index)
        {
            self.shown = known.1;
            self.shapes.push_back(known);
            return true;
        }
        let serial = self.next;
        self.next = self.next.wrapping_add(1).max(HIDDEN_CURSOR + 1);
        if let Some(message) = shape_message(&key, serial) {
            news.push(CursorNews::Message(message));
        }
        if self.shapes.len() == CURSOR_SHAPES {
            self.shapes.pop_front();
        }
        self.shapes.push_back((key, serial));
        self.shown = serial;
        true
    }
}

fn shape_message(shape: &Shape, serial: u32) -> Option<ServerMessage> {
    Some(ServerMessage::CursorShape(CursorShape {
        serial,
        width: u16::try_from(shape.size.0).ok()?,
        height: u16::try_from(shape.size.1).ok()?,
        hotspot_x: i16::try_from(shape.hotspot.0).ok()?,
        hotspot_y: i16::try_from(shape.hotspot.1).ok()?,
        rgba: shape.rgba.clone(),
    }))
}

fn to_rgba(pixels: &[u8], stride: usize, (width, height): (u32, u32), bgr: bool) -> Vec<u8> {
    let row = width as usize * 4;
    let mut rgba = Vec::with_capacity(row * height as usize);
    for line in pixels.chunks(stride).take(height as usize) {
        for [first, green, third, opacity] in line[..row.min(line.len())].as_chunks::<4>().0 {
            let (red, blue) = match bgr {
                true => (*third, *first),
                false => (*first, *third),
            };
            rgba.extend_from_slice(&[
                straight(red, *opacity),
                straight(*green, *opacity),
                straight(blue, *opacity),
                *opacity,
            ]);
        }
    }
    rgba
}

fn straight(channel: u8, opacity: u8) -> u8 {
    match opacity {
        0 => 0,
        u8::MAX => channel,
        _ => ((u32::from(channel) * 255 + u32::from(opacity) / 2) / u32::from(opacity)).min(255)
            as u8,
    }
}
