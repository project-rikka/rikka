mod file;
mod motion;

use animate_prop::{PropKey, PropSetting};
use rikka_mmd::VmdCurve;

fn key(setting: PropSetting, frame: u32, value: f32) -> PropKey {
    PropKey {
        setting,
        frame,
        value,
        curve: VmdCurve::LINEAR,
    }
}
