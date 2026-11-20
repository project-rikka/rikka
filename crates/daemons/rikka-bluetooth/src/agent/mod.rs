mod prompts;

pub(crate) use prompts::Prompts;

use std::sync::Arc;

use zbus::zvariant::OwnedObjectPath;

use crate::device::DeviceId;
use crate::prompt::{Question, Reply};

pub(crate) const PATH: &str = "/org/rikka/settings/bluetooth/agent";
pub(crate) const CAPABILITY: &str = "KeyboardDisplay";

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.bluez.Error")]
pub(crate) enum Refusal {
    Rejected(String),
    Canceled(String),
}

pub(crate) struct Agent {
    prompts: Arc<Prompts>,
}

impl Agent {
    pub(crate) fn new(prompts: Arc<Prompts>) -> Self {
        Self { prompts }
    }

    async fn wait(&self, device: OwnedObjectPath, question: Question) -> Result<Reply, Refusal> {
        let reply = self
            .prompts
            .ask(DeviceId(device.as_str().to_owned()), question);
        reply
            .await
            .map_err(|_| Refusal::Canceled("nobody answered".to_owned()))
    }

    async fn agreed(&self, device: OwnedObjectPath, question: Question) -> Result<(), Refusal> {
        match self.wait(device, question).await? {
            Reply::Yes => Ok(()),
            _ => Err(Refusal::Rejected("the person said no".to_owned())),
        }
    }
}

#[zbus::interface(name = "org.bluez.Agent1")]
impl Agent {
    async fn release(&self) {
        self.prompts.withdraw();
    }

    async fn request_pin_code(&self, device: OwnedObjectPath) -> Result<String, Refusal> {
        match self.wait(device, Question::AskPin).await? {
            Reply::Pin(pin) => Ok(pin),
            _ => Err(Refusal::Rejected("no code was given".to_owned())),
        }
    }

    async fn display_pin_code(
        &self,
        device: OwnedObjectPath,
        pincode: String,
    ) -> Result<(), Refusal> {
        self.prompts.notify(
            DeviceId(device.as_str().to_owned()),
            Question::DisplayPin(pincode),
        );
        Ok(())
    }

    async fn request_passkey(&self, device: OwnedObjectPath) -> Result<u32, Refusal> {
        match self.wait(device, Question::AskPasskey).await? {
            Reply::Passkey(passkey) => Ok(passkey),
            _ => Err(Refusal::Rejected("no number was given".to_owned())),
        }
    }

    async fn display_passkey(&self, device: OwnedObjectPath, passkey: u32, entered: u16) {
        self.prompts.notify(
            DeviceId(device.as_str().to_owned()),
            Question::Display { passkey, entered },
        );
    }

    async fn request_confirmation(
        &self,
        device: OwnedObjectPath,
        passkey: u32,
    ) -> Result<(), Refusal> {
        self.agreed(device, Question::Confirm(passkey)).await
    }

    async fn request_authorization(&self, device: OwnedObjectPath) -> Result<(), Refusal> {
        self.agreed(device, Question::Authorize).await
    }

    async fn authorize_service(
        &self,
        device: OwnedObjectPath,
        uuid: String,
    ) -> Result<(), Refusal> {
        self.agreed(device, Question::AuthorizeService(uuid)).await
    }

    async fn cancel(&self) {
        self.prompts.withdraw();
    }
}

impl std::fmt::Debug for Agent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Agent").finish_non_exhaustive()
    }
}
