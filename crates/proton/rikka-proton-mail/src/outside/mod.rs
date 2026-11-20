mod client;
mod message;
mod reply;
mod session;

pub use client::OutsideClient;
pub use message::{OutsideMessage, SentReply};
pub use reply::OutsideReply;
pub use session::OutsideSession;

use crate::error::Error;

const TOKEN: &str = "mail/v4/eo/token";
const MESSAGE: &str = "mail/v4/eo/message";
const ATTACHMENT: &str = "mail/v4/eo/attachment";
const REPLY: &str = "mail/v4/eo/reply";

fn password_decryption_error(error: rikka_proton_pgp::Error) -> Error {
    match error {
        rikka_proton_pgp::Error::DecryptionFailed(_) => Error::WrongPassword,
        other => Error::Pgp(other),
    }
}
