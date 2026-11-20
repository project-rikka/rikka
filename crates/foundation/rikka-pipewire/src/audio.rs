use libspa::param::ParamType;
use libspa::param::audio::{AudioFormat, AudioInfoRaw};
use libspa::sys;
use libspa::utils::SpaTypes;

use crate::object;

#[must_use]
pub fn float_format(rate: Option<u32>, channels: Option<u16>) -> Vec<u8> {
    let mut info = AudioInfoRaw::new();
    info.set_format(AudioFormat::F32LE);
    if let Some(rate) = rate {
        info.set_rate(rate);
    }
    if let Some(channels) = channels {
        info.set_channels(u32::from(channels));
        let layout = positions(channels);
        if !layout.is_empty() {
            let mut position = [0; sys::SPA_AUDIO_MAX_CHANNELS as usize];
            position[..layout.len()].copy_from_slice(layout);
            info.set_position(position);
        }
    }
    object(
        SpaTypes::ObjectParamFormat,
        ParamType::EnumFormat,
        info.into(),
    )
}

fn positions(channels: u16) -> &'static [u32] {
    match channels {
        1 => &[sys::SPA_AUDIO_CHANNEL_MONO],
        2 => &[sys::SPA_AUDIO_CHANNEL_FL, sys::SPA_AUDIO_CHANNEL_FR],
        4 => &[
            sys::SPA_AUDIO_CHANNEL_FL,
            sys::SPA_AUDIO_CHANNEL_FR,
            sys::SPA_AUDIO_CHANNEL_RL,
            sys::SPA_AUDIO_CHANNEL_RR,
        ],
        6 => &[
            sys::SPA_AUDIO_CHANNEL_FL,
            sys::SPA_AUDIO_CHANNEL_FR,
            sys::SPA_AUDIO_CHANNEL_FC,
            sys::SPA_AUDIO_CHANNEL_LFE,
            sys::SPA_AUDIO_CHANNEL_RL,
            sys::SPA_AUDIO_CHANNEL_RR,
        ],
        8 => &[
            sys::SPA_AUDIO_CHANNEL_FL,
            sys::SPA_AUDIO_CHANNEL_FR,
            sys::SPA_AUDIO_CHANNEL_FC,
            sys::SPA_AUDIO_CHANNEL_LFE,
            sys::SPA_AUDIO_CHANNEL_RL,
            sys::SPA_AUDIO_CHANNEL_RR,
            sys::SPA_AUDIO_CHANNEL_SL,
            sys::SPA_AUDIO_CHANNEL_SR,
        ],
        _ => &[],
    }
}
