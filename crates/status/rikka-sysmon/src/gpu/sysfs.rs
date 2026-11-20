use std::path::Path;

const DRM: &str = "/sys/class/drm";

#[derive(Clone, Debug)]
pub(crate) struct Device {
    pub(crate) card: String,
    pub(crate) slot: String,
    pub(crate) driver: String,
    pub(crate) vendor_id: u16,
    pub(crate) device_id: u16,
}

impl Device {
    pub(crate) fn path(&self, suffix: &str) -> String {
        format!("{DRM}/{}/device/{suffix}", self.card)
    }
}

pub(crate) fn devices() -> Vec<Device> {
    let Ok(entries) = std::fs::read_dir(DRM) else {
        return Vec::new();
    };
    let mut cards: Vec<String> = entries
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| {
            name.strip_prefix("card").is_some_and(|rest| {
                !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_digit())
            })
        })
        .collect();
    cards.sort();
    cards.into_iter().filter_map(device).collect()
}

fn device(card: String) -> Option<Device> {
    let device = format!("{DRM}/{card}/device");
    let slot = std::fs::canonicalize(&device)
        .ok()?
        .file_name()?
        .to_str()?
        .to_owned();
    let driver = std::fs::canonicalize(format!("{device}/driver"))
        .ok()
        .and_then(|path| Some(path.file_name()?.to_str()?.to_owned()))
        .unwrap_or_default();
    Some(Device {
        vendor_id: hex(&format!("{device}/vendor")),
        device_id: hex(&format!("{device}/device")),
        card,
        slot,
        driver,
    })
}

pub(crate) fn temperature(device: &Device) -> Option<f32> {
    hwmon(device, &["temp1_input"]).map(|millidegrees| millidegrees / 1000.0)
}

pub(crate) fn power(device: &Device) -> Option<f32> {
    hwmon(device, &["power1_average", "power1_input"]).map(|microwatts| microwatts / 1e6)
}

fn hwmon(device: &Device, files: &[&str]) -> Option<f32> {
    let entries = std::fs::read_dir(device.path("hwmon")).ok()?;
    entries.flatten().find_map(|entry| {
        files
            .iter()
            .find_map(|file| read::<f32>(entry.path().join(file)))
    })
}

pub(crate) fn read<T: std::str::FromStr>(path: impl AsRef<Path>) -> Option<T> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn hex(path: &str) -> u16 {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|value| u16::from_str_radix(value.trim().trim_start_matches("0x"), 16).ok())
        .unwrap_or_default()
}
