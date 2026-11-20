use crate::eval::{hsv_to_rgb, rgb_to_hsv};
use crate::node::{PortSpec, out};
use crate::ops::{Category, Op, OpInfo};
use crate::types::{Constant, PortType, ValueType};

const COLOR: PortSpec = PortSpec::color("color", 0.8, 0.8, 0.8);

pub(crate) fn info(op: Op) -> OpInfo {
    let color = [out("value", PortType::Value(ValueType::Float3))];
    let factor = PortSpec::number("factor", 1.0).range(0.0, 1.0);
    match op {
        Op::HueSaturation => OpInfo::new(
            "Hue saturation value",
            Category::Color,
            "Blender's adjustment: hue turned by hue - 0.5 turns, saturation scaled (clamped to 1), value scaled, mixed in by factor.",
            &[
                COLOR,
                PortSpec::number("hue", 0.5).range(0.0, 1.0),
                PortSpec::number("saturation", 1.0).soft(0.0, 2.0),
                PortSpec::number("value", 1.0).soft(0.0, 2.0),
                factor,
            ],
            &color,
        ),
        Op::BrightnessContrast => OpInfo::new(
            "Brightness contrast",
            Category::Color,
            "Blender's max((1 + contrast) × color + brightness - contrast/2, 0).",
            &[
                COLOR,
                PortSpec::number("brightness", 0.0).soft(-1.0, 1.0),
                PortSpec::number("contrast", 0.0).soft(-1.0, 1.0),
            ],
            &color,
        ),
        Op::Gamma => OpInfo::new(
            "Gamma",
            Category::Color,
            "Each positive channel raised to gamma; others pass unchanged.",
            &[COLOR, PortSpec::number("gamma", 1.0).soft(0.0, 4.0)],
            &color,
        ),
        Op::Invert => OpInfo::new(
            "Invert",
            Category::Color,
            "1 - color, mixed in by factor.",
            &[COLOR, factor],
            &color,
        ),
        Op::Blackbody => OpInfo::new(
            "Blackbody",
            Category::Color,
            "The linear Rec. 709 colour of a black body at temperature kelvin (Planckian locus, 1667-25000 K) at luminance 1.",
            &[PortSpec::number("temperature", 6500.0).soft(1000.0, 12000.0)],
            &color,
        ),
        Op::Wavelength => OpInfo::new(
            "Wavelength",
            Category::Color,
            "The linear Rec. 709 colour of monochromatic light of wavelength nanometres (CIE 1931 fit), negative channels clipped.",
            &[PortSpec::number("wavelength", 500.0).soft(380.0, 780.0)],
            &color,
        ),
        _ => unreachable!("{op:?} is not a colour op"),
    }
}

pub(crate) fn expression(op: Op, args: &[String]) -> String {
    let call = |name: &str| format!("{name}({})", args.join(", "));
    match op {
        Op::HueSaturation => call("rikka_graph_hue_saturation"),
        Op::BrightnessContrast => call("rikka_graph_brightness_contrast"),
        Op::Gamma => call("rikka_graph_gamma"),
        Op::Invert => format!("lerp({0}, 1.0 - {0}, {1})", args[0], args[1]),
        Op::Blackbody => call("rikka_graph_blackbody"),
        Op::Wavelength => call("rikka_graph_wavelength"),
        _ => unreachable!("{op:?} is not a colour op"),
    }
}

fn xyz_to_rgb(x: f32, y: f32, z: f32) -> [f32; 3] {
    [
        (3.240_454_2 * x - 1.537_138_5 * y - 0.498_531_4 * z).max(0.0),
        (-0.969_266 * x + 1.876_010_8 * y + 0.041_556 * z).max(0.0),
        (0.055_643_4 * x - 0.204_025_9 * y + 1.057_225_2 * z).max(0.0),
    ]
}

pub(crate) fn blackbody(kelvin: f32) -> [f32; 3] {
    let t = kelvin.clamp(1667.0, 25000.0);
    let t1 = 1000.0 / t;
    let x = match t <= 4000.0 {
        true => ((-0.266_123_9 * t1 - 0.234_358_9) * t1 + 0.877_695_6) * t1 + 0.179_910,
        false => ((-3.025_847 * t1 + 2.107_037_9) * t1 + 0.222_634_7) * t1 + 0.240_390,
    };
    let y = if t <= 2222.0 {
        ((-1.106_381_4 * x - 1.348_110_2) * x + 2.185_558_3) * x - 0.202_196_83
    } else if t <= 4000.0 {
        ((-0.954_947_6 * x - 1.374_185_9) * x + 2.091_37) * x - 0.167_488_67
    } else {
        ((3.081_758 * x - 5.873_387) * x + 3.751_13) * x - 0.370_014_83
    };
    xyz_to_rgb(x / y, 1.0, (1.0 - x - y) / y)
}

fn lobe(wavelength: f32, center: f32, below: f32, above: f32) -> f32 {
    let spread = if wavelength < center { below } else { above };
    let t = (wavelength - center) / spread;
    (-0.5 * t * t).exp()
}

pub(crate) fn wavelength(l: f32) -> [f32; 3] {
    let x = 1.056 * lobe(l, 599.8, 37.9, 31.0) + 0.362 * lobe(l, 442.0, 16.0, 26.7)
        - 0.065 * lobe(l, 501.1, 20.4, 26.2);
    let y = 0.821 * lobe(l, 568.8, 46.9, 40.5) + 0.286 * lobe(l, 530.9, 16.3, 31.1);
    let z = 1.217 * lobe(l, 437.0, 11.8, 36.0) + 0.681 * lobe(l, 459.0, 26.0, 13.8);
    xyz_to_rgb(x, y, z)
}

pub(crate) fn compute(op: Op, args: &[Constant]) -> Vec<Constant> {
    let rgb = |value: &Constant| [value.component(0), value.component(1), value.component(2)];
    let s = |index: usize| args[index].component(0);
    let color = match op {
        Op::HueSaturation => {
            let original = rgb(&args[0]);
            let mut hsv = rgb_to_hsv(original);
            let turned = hsv[0] + s(1) + 0.5;
            hsv[0] = turned - turned.floor();
            hsv[1] = (hsv[1] * s(2)).clamp(0.0, 1.0);
            hsv[2] *= s(3);
            let adjusted = hsv_to_rgb(hsv);
            [0, 1, 2].map(|i| original[i] + (adjusted[i] - original[i]) * s(4))
        }
        Op::BrightnessContrast => {
            let a = 1.0 + s(2);
            let b = s(1) - s(2) * 0.5;
            rgb(&args[0]).map(|channel| (a * channel + b).max(0.0))
        }
        Op::Gamma => rgb(&args[0]).map(|channel| match channel > 0.0 {
            true => channel.powf(s(1)),
            false => channel,
        }),
        Op::Invert => rgb(&args[0]).map(|channel| channel + (1.0 - channel - channel) * s(1)),
        Op::Blackbody => blackbody(s(0)),
        Op::Wavelength => wavelength(s(0)),
        _ => unreachable!("{op:?} is not a colour op"),
    };
    vec![Constant::vec3(color[0], color[1], color[2])]
}
