use crate::device::DeviceId;
use crate::error::{Error, Result};
use crate::profile::dbus;
use crate::ssid::Ssid;
use crate::values::{self, Draft};

const PASSPHRASE: std::ops::RangeInclusive<usize> = 8..=63;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HotspotBand {
    #[default]
    Automatic,
    Legacy,
    Fast,
}

impl HotspotBand {
    fn name(self) -> Option<&'static str> {
        match self {
            Self::Automatic => None,
            Self::Legacy => Some("bg"),
            Self::Fast => Some("a"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Hotspot {
    pub device: DeviceId,
    pub name: Ssid,
    pub passphrase: String,
    pub band: HotspotBand,
}

impl Hotspot {
    pub(crate) fn draft(&self, user: &str) -> Result<Draft> {
        if self.name.is_empty() {
            return Err(Error::invalid("the name of a hotspot", "it is empty"));
        }
        if !PASSPHRASE.contains(&self.passphrase.len()) {
            return Err(Error::invalid(
                "the passphrase of a hotspot",
                format!(
                    "WPA2 takes between {} and {} characters",
                    PASSPHRASE.start(),
                    PASSPHRASE.end()
                ),
            ));
        }

        let mut draft = Draft::new();
        values::put(
            &mut draft,
            dbus::CONNECTION,
            "id",
            self.name.name().into_owned(),
        );
        values::put(
            &mut draft,
            dbus::CONNECTION,
            "type",
            dbus::WIRELESS.to_owned(),
        );
        values::put(&mut draft, dbus::CONNECTION, "autoconnect", false);
        values::put(
            &mut draft,
            dbus::CONNECTION,
            "permissions",
            dbus::permissions(false, user),
        );

        values::put(
            &mut draft,
            dbus::WIRELESS,
            "ssid",
            self.name.as_bytes().to_vec(),
        );
        values::put(&mut draft, dbus::WIRELESS, "mode", "ap".to_owned());
        if let Some(band) = self.band.name() {
            values::put(&mut draft, dbus::WIRELESS, "band", band.to_owned());
        }

        values::put(&mut draft, dbus::SECURITY, "key-mgmt", "wpa-psk".to_owned());
        values::put(&mut draft, dbus::SECURITY, "psk", self.passphrase.clone());
        values::put(&mut draft, dbus::SECURITY, "proto", vec!["rsn".to_owned()]);
        values::put(
            &mut draft,
            dbus::SECURITY,
            "pairwise",
            vec!["ccmp".to_owned()],
        );
        values::put(&mut draft, dbus::SECURITY, "group", vec!["ccmp".to_owned()]);

        values::put(&mut draft, dbus::IPV4, "method", "shared".to_owned());
        values::put(&mut draft, dbus::IPV6, "method", "ignore".to_owned());
        Ok(draft)
    }
}
