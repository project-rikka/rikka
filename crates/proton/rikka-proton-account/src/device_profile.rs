use std::fs;
use std::path::Path;

use jiff::Timestamp;
use jiff::tz::TimeZone;
use serde::Serialize;

const CHALLENGE_VERSION: &str = "2.0.7";
const DEFAULT_LANGUAGE: &str = "en";
const BYTES_PER_GB: u64 = 1_000_000_000;
const SECTOR_BYTES: u64 = 512;
const VIRTUAL_DEVICES: &[&str] = &["loop", "ram", "zram", "dm-", "md", "sr", "nbd", "fd"];
const INPUT_METHOD_VARIABLES: &[&str] = &["GTK_IM_MODULE", "QT_IM_MODULE", "XMODIFIERS"];
const DEFAULT_FONT_SCALE: f32 = 1.0;
const DEFAULT_TIMEZONE: &str = "UTC";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeviceProfile {
    #[serde(rename = "v")]
    version: &'static str,
    #[serde(rename = "appLang")]
    language: String,
    #[serde(rename = "timezone")]
    timezone: String,
    #[serde(rename = "deviceName")]
    device_name: i64,
    #[serde(rename = "regionCode")]
    region: String,
    #[serde(rename = "timezoneOffset")]
    timezone_offset: i32,
    #[serde(rename = "isJailbreak")]
    jailbroken: bool,
    #[serde(rename = "preferredContentSize")]
    font_scale: String,
    #[serde(rename = "storageCapacity")]
    storage: f64,
    #[serde(rename = "isDarkmodeOn")]
    dark_mode: bool,
    #[serde(rename = "keyboards")]
    keyboards: Vec<String>,
}

impl DeviceProfile {
    #[must_use]
    pub fn collect() -> DeviceProfile {
        let locale = rikka_types::LocaleCategory::Messages.locale();
        let language = locale.as_ref().map_or_else(
            || DEFAULT_LANGUAGE.to_owned(),
            |locale| locale.language().to_ascii_lowercase(),
        );
        let region = locale
            .as_ref()
            .and_then(|locale| locale.territory())
            .map(str::to_ascii_uppercase)
            .unwrap_or_default();
        let zone = TimeZone::system();
        let offset = zone.to_offset(Timestamp::now()).seconds();

        DeviceProfile {
            version: CHALLENGE_VERSION,
            language,
            timezone: zone.iana_name().unwrap_or(DEFAULT_TIMEZONE).to_owned(),
            device_name: rolling_hash(&machine_name()),
            region,
            timezone_offset: -offset / 60,
            jailbroken: false,
            font_scale: format!("{DEFAULT_FONT_SCALE:?}"),
            storage: storage_gigabytes(Path::new("/sys/block")),
            dark_mode: false,
            keyboards: input_methods(),
        }
    }

    #[must_use]
    pub fn dark_mode(mut self, dark_mode: bool) -> DeviceProfile {
        self.dark_mode = dark_mode;
        self
    }

    #[must_use]
    pub fn font_scale(mut self, scale: f32) -> DeviceProfile {
        self.font_scale = format!("{scale:?}");
        self
    }

    #[must_use]
    pub fn language(&self) -> &str {
        &self.language
    }

    #[must_use]
    pub fn region(&self) -> &str {
        &self.region
    }

    #[must_use]
    pub fn timezone(&self) -> &str {
        &self.timezone
    }

    #[must_use]
    pub fn timezone_offset(&self) -> i32 {
        self.timezone_offset
    }
}

fn machine_name() -> String {
    let read_trimmed = |path: &str| {
        fs::read_to_string(path)
            .map(|text| text.trim().to_owned())
            .unwrap_or_default()
    };

    format!(
        "{}/{} {}",
        read_trimmed("/sys/class/dmi/id/product_name"),
        read_trimmed("/sys/class/dmi/id/sys_vendor"),
        read_trimmed("/proc/sys/kernel/hostname"),
    )
}

fn rolling_hash(text: &str) -> i64 {
    const BASE: i64 = 7;
    const MODULUS: i64 = 100_000_007;

    let mut hash: i64 = 0;
    let mut coefficient: i64 = 0;

    for byte in text.bytes() {
        coefficient = match coefficient {
            0 => 1,
            accumulated => accumulated * BASE % MODULUS,
        };
        hash = hash.wrapping_add(i64::from(byte as i8) * coefficient);
    }

    hash
}

fn storage_gigabytes(blocks: &Path) -> f64 {
    let Ok(devices) = fs::read_dir(blocks) else {
        return 0.0;
    };

    let bytes: u64 = devices
        .filter_map(|device| device.ok())
        .filter(|device| {
            let name = device.file_name();
            let name = name.to_string_lossy();
            !VIRTUAL_DEVICES
                .iter()
                .any(|prefix| name.starts_with(prefix))
        })
        .filter(|device| {
            fs::read_to_string(device.path().join("removable"))
                .map(|removable| removable.trim() == "0")
                .unwrap_or(false)
        })
        .filter_map(|device| fs::read_to_string(device.path().join("size")).ok())
        .filter_map(|sectors| sectors.trim().parse::<u64>().ok())
        .map(|sectors| sectors.saturating_mul(SECTOR_BYTES))
        .sum();

    (bytes / BYTES_PER_GB) as f64
}

fn input_methods() -> Vec<String> {
    let mut methods: Vec<String> = Vec::new();

    for variable in INPUT_METHOD_VARIABLES {
        let Ok(value) = std::env::var(variable) else {
            continue;
        };
        let name = value.trim_start_matches("@im=").trim().to_owned();
        if !name.is_empty() && !methods.contains(&name) {
            methods.push(name);
        }
    }

    methods
}
