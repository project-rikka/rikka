use std::collections::HashMap;

use rikka_gecko_protocol::{EmbedderMessage, Target};

#[derive(Debug, Default)]
pub(crate) struct Gates {
    hold: HashMap<Target, Vec<EmbedderMessage>>,
}

impl Gates {
    pub(crate) fn hold(&mut self, target: Target) {
        self.hold.entry(target).or_default();
    }

    pub(crate) fn filter(&mut self, message: EmbedderMessage) -> Option<EmbedderMessage> {
        let gate = match &message {
            EmbedderMessage::Event(event) if self.hold.contains_key(&event.target) => {
                event.target.clone()
            }
            EmbedderMessage::Reply(_) | EmbedderMessage::Release(_) => return Some(message),
            EmbedderMessage::Listen(listen) | EmbedderMessage::Unlisten(listen)
                if listen.target == Target::Runtime =>
            {
                return Some(message);
            }
            _ => Target::Runtime,
        };
        match self.hold.get_mut(&gate) {
            Some(queue) => {
                queue.push(message);
                None
            }
            None => Some(message),
        }
    }

    pub(crate) fn release(&mut self, target: &Target) -> Vec<EmbedderMessage> {
        self.hold.remove(target).unwrap_or_default()
    }
}
