use alsa::Direction;
use alsa::card::Iter;
use alsa::ctl::{Ctl, DeviceIter};
use rikka_hifi_core::DeviceId;

use crate::device::Device;
use crate::exclusive::mixer::HardwareVolume;
use crate::exclusive::probe;

pub(crate) fn list(probed: bool) -> Result<Vec<Device>, alsa::Error> {
    let mut devices = Vec::new();
    for card in Iter::new() {
        let card = card?;
        let Ok(index) = u32::try_from(card.get_index()) else {
            continue;
        };
        let Ok(control) = Ctl::from_card(&card, false) else {
            continue;
        };
        let Ok(info) = control.card_info() else {
            continue;
        };
        let card_id = info.get_id().unwrap_or_default().to_owned();
        let name = info.get_name().unwrap_or_default().to_owned();
        let mixer = probed
            .then(|| HardwareVolume::find(index).map(|volume| volume.name().to_owned()))
            .flatten();
        for device in DeviceIter::new(&control) {
            let Ok(device) = u32::try_from(device) else {
                continue;
            };
            let Ok(pcm) = control.pcm_info(device, 0, Direction::Playback) else {
                continue;
            };
            devices.push(Device {
                pcm: pcm.get_name().unwrap_or_default().to_owned(),
                id: DeviceId::new(card_id.clone(), device),
                card: index,
                name: name.clone(),
                capabilities: probed.then(|| probe::capabilities(index, device)).flatten(),
                mixer: mixer.clone(),
            });
        }
    }
    Ok(devices)
}

pub(crate) fn find(id: &DeviceId) -> Option<Device> {
    list(false)
        .ok()?
        .into_iter()
        .find(|device| device.id == *id)
}
