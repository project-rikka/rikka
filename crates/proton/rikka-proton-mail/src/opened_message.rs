use crate::decrypted::DecryptedMessage;
use crate::message::Message;

#[derive(Debug, Clone)]
pub struct OpenedMessage {
    message: Message,
    decrypted: DecryptedMessage,
}

impl OpenedMessage {
    pub(crate) fn new(message: Message, decrypted: DecryptedMessage) -> OpenedMessage {
        OpenedMessage { message, decrypted }
    }

    #[must_use]
    pub fn message(&self) -> &Message {
        &self.message
    }

    #[must_use]
    pub fn decrypted(&self) -> &DecryptedMessage {
        &self.decrypted
    }

    #[must_use]
    pub fn into_parts(self) -> (Message, DecryptedMessage) {
        (self.message, self.decrypted)
    }
}
