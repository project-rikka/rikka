use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use bytes::{Buf, BytesMut};

use crate::sse::event::{DEFAULT_TYPE, Event};

const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

#[derive(Debug, Default)]
pub struct EventDecoder {
    bytes: BytesMut,
    scanned: usize,
    data: String,
    event: String,
    id: Option<Arc<str>>,
    last_event_id: Option<Arc<str>>,
    reconnection_time: Option<Duration>,
    ready: VecDeque<Event>,
    started: bool,
}

impl EventDecoder {
    #[must_use]
    pub fn new() -> EventDecoder {
        EventDecoder::default()
    }

    pub fn push(&mut self, chunk: &[u8]) {
        self.bytes.extend_from_slice(chunk);

        if !self.started {
            if self.bytes.len() < BOM.len() && self.bytes[..] == BOM[..self.bytes.len()] {
                return;
            }
            if self.bytes.starts_with(BOM) {
                self.bytes.advance(BOM.len());
            }
            self.started = true;
        }

        while let Some(line) = self.take_line() {
            self.line(&line);
        }
    }

    #[must_use]
    pub fn next_event(&mut self) -> Option<Event> {
        self.ready.pop_front()
    }

    #[must_use]
    pub fn last_event_id(&self) -> Option<&str> {
        self.last_event_id.as_deref()
    }

    #[must_use]
    pub fn reconnection_time(&self) -> Option<Duration> {
        self.reconnection_time
    }

    fn take_line(&mut self) -> Option<String> {
        let Some(end) = self.bytes[self.scanned..]
            .iter()
            .position(|byte| *byte == b'\r' || *byte == b'\n')
            .map(|offset| self.scanned + offset)
        else {
            self.scanned = self.bytes.len();
            return None;
        };

        let carriage_return = self.bytes[end] == b'\r';
        if carriage_return && end + 1 == self.bytes.len() {
            self.scanned = end;
            return None;
        }

        let skip = usize::from(carriage_return && self.bytes[end + 1] == b'\n') + 1;
        let line = String::from_utf8_lossy(&self.bytes[..end]).into_owned();

        self.bytes.advance(end + skip);
        self.scanned = 0;
        Some(line)
    }

    fn line(&mut self, line: &str) {
        if line.is_empty() {
            self.dispatch();
            return;
        }
        if line.starts_with(':') {
            return;
        }

        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (line, ""),
        };

        match field {
            "event" => {
                self.event.clear();
                self.event.push_str(value);
            }
            "data" => {
                self.data.push_str(value);
                self.data.push('\n');
            }
            "id" if !value.contains('\0') => self.id = Some(Arc::from(value)),
            "retry" => {
                if let Ok(milliseconds) = value.parse() {
                    self.reconnection_time = Some(Duration::from_millis(milliseconds));
                }
            }
            _ => {}
        }
    }

    fn dispatch(&mut self) {
        self.last_event_id.clone_from(&self.id);
        if self.data.is_empty() {
            self.event.clear();
            return;
        }

        let mut data = std::mem::take(&mut self.data);
        if data.ends_with('\n') {
            data.pop();
        }

        let event = match self.event.is_empty() {
            true => DEFAULT_TYPE.to_owned(),
            false => std::mem::take(&mut self.event),
        };

        self.ready
            .push_back(Event::new(event, data, self.id.clone()));
    }
}
