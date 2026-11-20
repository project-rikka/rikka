use std::collections::VecDeque;

use futures_util::StreamExt;

use crate::error::Result;
use crate::event::Event;
use crate::stream::Stream;
use crate::transport::Events;

pub(crate) trait Assemble: Send + 'static {
    fn feed(&mut self, data: &str) -> Result<Vec<Event>>;

    fn finish(&mut self) -> Option<Event>;
}

struct Assembling<A> {
    frames: Events,
    assembler: A,
    ready: VecDeque<Event>,
    ended: bool,
}

pub(crate) fn assemble(frames: Events, assembler: impl Assemble) -> Stream {
    let state = Assembling {
        frames,
        assembler,
        ready: VecDeque::new(),
        ended: false,
    };

    Stream::new(futures_util::stream::unfold(
        state,
        |mut state| async move {
            loop {
                if let Some(event) = state.ready.pop_front() {
                    return Some((Ok(event), state));
                }
                if state.ended {
                    return None;
                }

                let Some(frame) = state.frames.next().await else {
                    state.ended = true;
                    let last = state.assembler.finish();
                    return last.map(|event| (Ok(event), state));
                };

                let events = match frame.and_then(|frame| state.assembler.feed(frame.data())) {
                    Ok(events) => events,
                    Err(error) => {
                        state.ended = true;
                        return Some((Err(error), state));
                    }
                };

                state.ready.extend(events);
            }
        },
    ))
}
