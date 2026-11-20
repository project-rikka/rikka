use std::str::FromStr;

const CARD: &str = "hw:CARD=";
const DEVICE: &str = ",DEV=";

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceId {
    pub card: String,
    pub device: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} names no hw:CARD=<card>,DEV=<n> device")]
pub struct NotDevice(pub String);

impl DeviceId {
    #[must_use]
    pub fn new(card: impl Into<String>, device: u32) -> DeviceId {
        DeviceId {
            card: card.into(),
            device,
        }
    }

    #[must_use]
    pub fn pcm(&self) -> String {
        format!("{CARD}{}{DEVICE}{}", self.card, self.device)
    }
}

impl FromStr for DeviceId {
    type Err = NotDevice;

    fn from_str(pcm: &str) -> Result<DeviceId, NotDevice> {
        pcm.strip_prefix(CARD)
            .and_then(|rest| rest.rsplit_once(DEVICE))
            .filter(|(card, _)| !card.is_empty())
            .and_then(|(card, device)| Some(DeviceId::new(card, device.parse().ok()?)))
            .ok_or_else(|| NotDevice(pcm.to_owned()))
    }
}

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.pcm())
    }
}
