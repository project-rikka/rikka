use std::future::Future;
use std::sync::Arc;

use serde::Deserialize;

use crate::channel::Link;
use crate::error::Result;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncedTab {
    pub url: String,
    #[serde(default)]
    pub title: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub name: String,
    pub tabs: Vec<SyncedTab>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireDevice {
    device_name: String,
    #[serde(default)]
    sessions: Vec<WireSession>,
}

#[derive(Deserialize)]
struct WireSession {
    window: Option<WireWindow>,
    tab: Option<SyncedTab>,
}

#[derive(Deserialize)]
struct WireWindow {
    #[serde(default)]
    tabs: Vec<SyncedTab>,
}

#[derive(Clone, Debug)]
pub struct Devices {
    link: Arc<Link>,
}

impl Devices {
    pub(crate) fn new(link: Arc<Link>) -> Devices {
        Devices { link }
    }

    pub fn list(&self) -> impl Future<Output = Result<Vec<Device>>> + Send + use<> {
        let called = self.link.call("sessions.getDevices", Vec::new());
        async move {
            let devices: Vec<WireDevice> = called.decoded().await?;
            Ok(devices
                .into_iter()
                .map(|device| Device {
                    name: device.device_name,
                    tabs: device
                        .sessions
                        .into_iter()
                        .flat_map(|session| {
                            session
                                .window
                                .map(|window| window.tabs)
                                .unwrap_or_default()
                                .into_iter()
                                .chain(session.tab)
                        })
                        .collect(),
                })
                .collect())
        }
    }
}
