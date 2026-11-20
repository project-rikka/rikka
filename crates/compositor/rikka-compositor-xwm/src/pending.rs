use std::collections::VecDeque;
use std::rc::Rc;

use x11rb_protocol::SequenceNumber;
use x11rb_protocol::protocol::xproto::{Atom, SelectionRequestEvent, Window};

use crate::poller::SourceId;
use crate::properties::Field;
use crate::selection::{Access, SelectionKind};
use crate::window::XWindowId;

#[derive(Debug)]
pub(crate) enum ExpectedReply {
    Property {
        window: XWindowId,
        epoch: u64,
        field: Field,
        quiet: bool,
    },
    Pid {
        window: XWindowId,
        epoch: u64,
    },
    Geometry {
        window: XWindowId,
        epoch: u64,
    },
    AtomName {
        atom: Atom,
    },
    Atom {
        name: Rc<str>,
    },
    Targets {
        kind: SelectionKind,
        generation: u64,
    },
    Chunk {
        source: SourceId,
    },
    DragTypes {
        source: Window,
    },
    Probe {
        attempt: u64,
        window: Window,
        field: Field,
    },
}

#[derive(Debug)]
pub(crate) enum Resume {
    Targets {
        kind: SelectionKind,
        generation: u64,
        atoms: Vec<Atom>,
    },
    DragTypes {
        source: Window,
        atoms: Vec<Atom>,
    },
    Request {
        request: SelectionRequestEvent,
        access: Access,
    },
    DragEnter {
        attempt: u64,
    },
}

#[derive(Debug)]
struct Entry {
    sequence: SequenceNumber,
    awaiting: ExpectedReply,
    then: Vec<Resume>,
}

#[derive(Debug, Default)]
pub(crate) struct Pending {
    entries: VecDeque<Entry>,
}

impl Pending {
    pub(crate) fn expect(&mut self, sequence: SequenceNumber, awaiting: ExpectedReply) {
        self.entries.push_back(Entry {
            sequence,
            awaiting,
            then: Vec::new(),
        });
    }

    pub(crate) fn after(&mut self, sequence: SequenceNumber, resume: Resume) {
        let index = self
            .entries
            .binary_search_by_key(&sequence, |entry| entry.sequence)
            .expect("a resumption waits on a request that is still unanswered");
        self.entries[index].then.push(resume);
    }

    pub(crate) fn next(&self) -> Option<SequenceNumber> {
        self.entries.front().map(|entry| entry.sequence)
    }

    pub(crate) fn take(&mut self) -> Option<(ExpectedReply, Vec<Resume>)> {
        self.entries
            .pop_front()
            .map(|entry| (entry.awaiting, entry.then))
    }
}
