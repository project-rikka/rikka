use std::borrow::Cow;
use std::os::fd::{AsFd, OwnedFd};
use std::time::Instant;

use rustix::buffer::spare_capacity;
use x11rb_protocol::protocol::xproto::{
    Atom, ChangeWindowAttributesAux, ChangeWindowAttributesRequest, EventMask,
};

use crate::fault::Outcome;
use crate::link::{Link, SelectionReply};
use crate::poller::{Direction, Poller, SourceId};
use crate::selection::flow::{Delivery, Outgoing};
use crate::selection::kind::SelectionKind;
use crate::selection::timeout::deadline;

#[derive(Debug)]
pub(crate) struct OutgoingTransfer {
    pub(crate) kind: SelectionKind,
    pub(crate) reply: SelectionReply,
    pub(crate) source: SourceId,
    pub(crate) deadline: Instant,
    data_type: Atom,
    requestor_events: EventMask,
    pipe: OwnedFd,
    flow: Outgoing,
    watching: bool,
    handle_reply: bool,
}

impl OutgoingTransfer {
    pub(crate) fn start(
        kind: SelectionKind,
        reply: SelectionReply,
        requestor_events: EventMask,
        pipe: OwnedFd,
        poller: &mut Poller,
        chunk: usize,
    ) -> std::io::Result<Self> {
        let source = poller.source();
        poller.watch(pipe.as_fd(), source, Direction::Read)?;
        Ok(Self {
            kind,
            reply,
            source,
            deadline: deadline(),
            data_type: reply.target,
            requestor_events,
            pipe,
            flow: Outgoing::new(chunk),
            watching: true,
            handle_reply: false,
        })
    }

    pub(crate) fn serves(&self, requestor: u32, property: Atom) -> bool {
        self.reply.requestor == requestor && self.reply.property == property
    }

    pub(crate) fn readable(&mut self, link: &mut Link, poller: &Poller) -> Outcome<bool> {
        while self.flow.wants_input() {
            let delivery = match rustix::io::read(&self.pipe, spare_capacity(self.flow.space())) {
                Ok(length) => self.flow.received(length),
                Err(rustix::io::Errno::AGAIN) => break,
                Err(rustix::io::Errno::INTR) => continue,
                Err(error) => {
                    tracing::debug!("Wayland selection source failed during transfer: {error}");
                    self.abort(link, poller)?;
                    return Ok(true);
                }
            };
            self.deadline = deadline();
            if let Some(delivery) = delivery {
                self.deliver(link, delivery)?;
            }
        }
        Ok(self.flow.finished())
    }

    pub(crate) fn deleted(&mut self, link: &mut Link, poller: &Poller) -> Outcome<bool> {
        if let Some(delivery) = self.flow.deleted() {
            self.deadline = deadline();
            self.deliver(link, delivery)?;
        }
        self.readable(link, poller)
    }

    pub(crate) fn abort(&mut self, link: &mut Link, poller: &Poller) -> Outcome {
        self.release(poller);
        if !self.handle_reply {
            self.handle_reply = true;
            link.reply_selection(self.reply, false)?;
        }
        Ok(())
    }

    pub(crate) fn release(&mut self, poller: &Poller) {
        if self.watching {
            poller.unwatch(self.pipe.as_fd());
            self.watching = false;
        }
    }

    fn deliver(&mut self, link: &mut Link, delivery: Delivery) -> Outcome {
        let reply = self.reply;
        match delivery {
            Delivery::Whole | Delivery::Chunk => {
                link.set_bytes(
                    reply.requestor,
                    reply.property,
                    self.data_type,
                    self.flow.data(),
                )?;
                self.flow.delivered();
                if !self.handle_reply {
                    self.handle_reply = true;
                    link.reply_selection(reply, true)?;
                }
            }
            Delivery::Begin(size) => {
                link.wire.send(ChangeWindowAttributesRequest {
                    window: reply.requestor,
                    value_list: Cow::Owned(
                        ChangeWindowAttributesAux::new().event_mask(self.requestor_events),
                    ),
                })?;
                link.set_words(
                    reply.requestor,
                    reply.property,
                    link.atoms.INCR,
                    &[u32::try_from(size).unwrap_or(u32::MAX)],
                )?;
                self.handle_reply = true;
                link.reply_selection(reply, true)?;
            }
        }
        Ok(())
    }
}
