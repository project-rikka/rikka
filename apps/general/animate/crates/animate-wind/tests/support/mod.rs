use animate_wind::{WindKey, WindSetting};
use rikka_mmd::VmdCurve;

pub(crate) fn key(setting: WindSetting, frame: u32, value: f32) -> WindKey {
    WindKey {
        setting,
        frame,
        value,
        curve: VmdCurve::LINEAR,
    }
}
