use rikka_app::prelude::*;
use rikka_widgets::column;

use rikka_network::{DeviceId, Hotspot, HotspotBand, Ssid};

use crate::picker::choose;
use crate::typed::secret;

const WIDE: f32 = 300.0;
const REVEAL: f32 = 40.0;

const GENERATED_PASSWORD_LENGTH: usize = 10;
const ALPHABET: &[u8] = b"abcdefghijkmnopqrstuvwxyz23456789";

const BANDS: [(HotspotBand, &str); 3] = [
    (HotspotBand::Automatic, "settings.network.hotspot.automatic"),
    (HotspotBand::Legacy, "settings.network.band.legacy"),
    (HotspotBand::Fast, "settings.network.band.fast"),
];

#[derive(State)]
pub(crate) struct HotspotForm {
    device: DeviceId,
    name: TextDocument,
    passphrase: TextDocument,
    band: HotspotBand,
    shown: bool,
}

impl HotspotForm {
    pub(crate) fn new(device: DeviceId, hostname: &str) -> Self {
        Self {
            device,
            name: TextDocument::new(hostname),
            passphrase: TextDocument::new(generate_password()),
            band: HotspotBand::default(),
            shown: true,
        }
    }

    pub(crate) fn is_ready(&self) -> bool {
        !self.name.is_empty() && self.passphrase.char_count() >= 8
    }

    pub(crate) fn request(&self) -> Hotspot {
        Hotspot {
            device: self.device.clone(),
            name: Ssid::from(self.name.text().as_str()),
            passphrase: self.passphrase.text(),
            band: self.band,
        }
    }
}

pub(crate) fn view(form: &HotspotForm) -> impl View {
    column! {
        field(
            tr!("settings.network.hotspot.name"),
            text_field(&form.name).on_change(|| {}).width(WIDE),
        );
        field(
            tr!("settings.network.hotspot.passphrase"),
            secret(
                &form.passphrase,
                tr!("settings.network.hotspot.passphrase"),
                form.shown,
                WIDE - REVEAL,
                || {},
                |form: &mut HotspotForm, shown: bool| form.shown = shown,
            ),
        );
        field(
            tr!("settings.network.hotspot.band"),
            choose(&BANDS, form.band, |form: &mut HotspotForm, band: HotspotBand| {
                form.band = band;
            }),
        );
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn generate_password() -> String {
    use std::io::Read;

    let mut bytes = [0_u8; GENERATED_PASSWORD_LENGTH];
    let drawn =
        std::fs::File::open("/dev/urandom").and_then(|mut random| random.read_exact(&mut bytes));
    if drawn.is_err() {
        return "rikka-hotspot".to_owned();
    }
    bytes
        .iter()
        .map(|byte| char::from(ALPHABET[usize::from(*byte) % ALPHABET.len()]))
        .collect()
}
