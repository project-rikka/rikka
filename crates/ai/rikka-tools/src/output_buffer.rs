use std::collections::VecDeque;

const TAIL: usize = 4;

#[derive(Debug)]
pub(crate) struct OutputBuffer {
    head: Vec<u8>,
    tail: VecDeque<u8>,
    retain: usize,
    keep: usize,
    total: usize,
    lines: usize,
    partial: bool,
    read: usize,
}

impl OutputBuffer {
    #[must_use]
    pub(crate) fn new(retain: usize) -> OutputBuffer {
        let retain = retain.max(1);
        OutputBuffer {
            head: Vec::new(),
            tail: VecDeque::new(),
            retain,
            keep: (retain / TAIL).max(1),
            total: 0,
            lines: 0,
            partial: false,
            read: 0,
        }
    }

    pub(crate) fn push(&mut self, bytes: &[u8]) {
        let Some(last) = bytes.last() else {
            return;
        };
        self.total += bytes.len();
        self.lines += newlines(bytes);
        self.partial = *last != b'\n';

        let room = self.retain.saturating_sub(self.head.len()).min(bytes.len());
        self.head.extend_from_slice(&bytes[..room]);

        let count = self.keep.min(bytes.len());
        while self.tail.len() + count > self.keep {
            self.tail.pop_front();
        }
        self.tail.extend(&bytes[bytes.len() - count..]);
    }

    #[must_use]
    pub(crate) fn summary(&self, room: usize) -> String {
        let room = room.max(1);
        if self.total == self.head.len() && self.total <= room {
            return text(&self.head);
        }

        let visible_head = &self.head[..room.min(self.head.len())];
        let left = self.lines - newlines(visible_head) + usize::from(self.partial);
        format!(
            "{}\n{}",
            text(visible_head),
            cut(left, self.total - visible_head.len())
        )
    }

    pub(crate) fn read_new(&mut self, room: usize) -> String {
        let room = room.max(1);

        if self.read < self.head.len() {
            let upto = self.head.len().min(self.read.saturating_add(room));
            let fresh = text(&self.head[self.read..upto]);
            self.read = upto;
            return fresh;
        }

        let retained = self.retained_len();
        let lost = retained.saturating_sub(self.read);
        let from = self.read.max(retained) - retained;
        let upto = self.tail.len().min(from.saturating_add(room));
        let fresh: Vec<u8> = self
            .tail
            .iter()
            .copied()
            .skip(from)
            .take(upto - from)
            .collect();
        self.read = retained + upto;

        match lost {
            0 => text(&fresh),
            lost => format!("{}\n{}", missing(lost), text(&fresh)),
        }
    }

    #[must_use]
    pub(crate) fn unread_len(&self) -> usize {
        let retained = self.retained_len();
        if self.head.len() >= retained {
            return self.total - self.read;
        }

        let head = self.head.len().saturating_sub(self.read);
        head + (self.total - self.read.max(retained))
    }

    fn retained_len(&self) -> usize {
        self.total - self.tail.len()
    }
}

fn newlines(bytes: &[u8]) -> usize {
    bytes.iter().filter(|byte| **byte == b'\n').count()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn cut(lines: usize, bytes: usize) -> String {
    let lines = match lines {
        1 => "1 line".to_owned(),
        lines => format!("{lines} lines"),
    };
    format!("… [{lines}, {bytes} bytes truncated] …")
}

fn missing(bytes: usize) -> String {
    format!("… [{bytes} bytes lost before this] …")
}
