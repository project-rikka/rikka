use animate_fluid::{EmitterKey, EmitterSetting, EmitterValue};
use rikka_mmd::VmdCurve;

pub(crate) fn key(setting: EmitterSetting, frame: u32, value: EmitterValue) -> EmitterKey {
    EmitterKey {
        setting,
        frame,
        value,
        curve: VmdCurve::LINEAR,
    }
}
