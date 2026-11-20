use rikka_renderer_3d::glam::{Mat3, Vec2, Vec3};
use rikka_renderer_3d::{
    BuiltinEffect, Composite, EffectInstance, Exposure, Metering, NITS_PER_RADIANCE_UNIT,
    Operation, Tone, ToneCurve, Tonemap, Value,
};

use crate::look::canvas::{CENTER, Canvas, Paint, Shape, encode};

fn flat(canvas: &Canvas, color: Vec3) -> Operation {
    canvas.paint(Paint::new(Shape::Flat, color)).into()
}

fn mapped(
    canvas: &mut Canvas,
    color: Vec3,
    operations: Vec<Operation>,
    tonemap: Tonemap,
) -> [u8; 3] {
    let mut composite = Composite::new();
    composite.push(flat(canvas, color));
    for operation in operations {
        composite.push(operation);
    }
    composite.push(tonemap);
    canvas.shoot(composite).rgb(CENTER, CENTER)
}

fn close(found: [u8; 3], expected: Vec3, tolerance: f32) -> bool {
    found
        .iter()
        .zip(expected.to_array())
        .all(|(&found, expected)| (f32::from(found) - encode(expected)).abs() <= tolerance)
}

fn hable(x: f32) -> f32 {
    let partial = |x: f32| {
        let (a, b, c, d, e, f) = (0.15, 0.50, 0.10, 0.20, 0.02, 0.30);
        (x * (a * x + c * b) + d * e) / (x * (a * x + b) + d * f) - e / f
    };
    partial(2.0 * x) / partial(11.2)
}

fn lottes(x: f32) -> f32 {
    let (contrast, shoulder, white, middle_in, middle_out) =
        (1.6_f32, 0.977_f32, 8.0_f32, 0.18_f32, 0.267_f32);
    let white_a = white.powf(contrast);
    let middle_a = middle_in.powf(contrast);
    let white_ad = white_a.powf(shoulder);
    let middle_ad = middle_a.powf(shoulder);
    let denominator = (white_ad - middle_ad) * middle_out;
    let b = (-middle_a + white_a * middle_out) / denominator;
    let c = (white_ad * middle_a - white_a * middle_ad * middle_out) / denominator;
    x.powf(contrast) / (x.powf(contrast * shoulder) * b + c)
}

fn khronos_neutral(x: f32) -> f32 {
    let start = 0.8 - 0.04;
    let desaturation = 0.15;
    let offset = match x < 0.08 {
        true => x - 6.25 * x * x,
        false => 0.04,
    };
    let x = x - offset;
    if x < start {
        return x;
    }
    let d = 1.0 - start;
    let peak = 1.0 - d * d / (x + d - start);
    let g = 1.0 - 1.0 / (desaturation * (x - peak) + 1.0);
    let scaled = x * peak / x;
    scaled + (peak - scaled) * g
}

fn uchimura(x: f32, curve: ToneCurve) -> f32 {
    let (peak, slope, toe_end, length, power) = (
        curve.peak,
        curve.contrast,
        curve.toe,
        curve.linear,
        curve.toe_power,
    );
    let span = (peak - toe_end) * length / slope;
    let shoulder_start = toe_end + span;
    let floor = toe_end + slope * span;
    let rate = -(slope * peak / (peak - floor)) / peak;
    let smooth = |t: f32| t * t * (3.0 - 2.0 * t);
    let toe_weight = 1.0 - smooth((x / toe_end).clamp(0.0, 1.0));
    let shoulder_weight = if x >= shoulder_start { 1.0 } else { 0.0 };
    let linear_weight = 1.0 - toe_weight - shoulder_weight;
    let toe = toe_end * (x / toe_end).powf(power);
    let shoulder = peak - (peak - floor) * (rate * (x - shoulder_start)).exp();
    let straight = toe_end + slope * (x - toe_end);
    toe * toe_weight + straight * linear_weight + shoulder * shoulder_weight
}

#[test]
fn tone_curves_hit_their_reference_values() {
    let mut canvas = Canvas::new();
    let curve = ToneCurve {
        peak: 1.0,
        contrast: 1.3,
        toe: 0.15,
        linear: 0.3,
        toe_power: 1.6,
    };
    let cases: [(Tonemap, &dyn Fn(f32) -> f32); 5] = [
        (Tonemap::new(Tone::Hable), &hable),
        (Tonemap::new(Tone::Reinhard), &|x| x / (1.0 + x)),
        (Tonemap::new(Tone::Lottes), &lottes),
        (Tonemap::new(Tone::Neutral), &khronos_neutral),
        (
            Tonemap {
                curve,
                ..Tonemap::new(Tone::Custom)
            },
            &move |x| uchimura(x, curve),
        ),
    ];
    for (tonemap, reference) in cases {
        for x in [0.05_f32, 0.18, 0.7, 3.0] {
            let found = mapped(&mut canvas, Vec3::splat(x), Vec::new(), tonemap);
            let expected = Vec3::splat(reference(x).clamp(0.0, 1.0));
            assert!(
                close(found, expected, 1.5),
                "{:?} at {x}: {found:?} instead of {}",
                tonemap.tone,
                encode(expected.x)
            );
        }
    }
}

fn hue(color: Vec3) -> f32 {
    let max = color.max_element();
    let min = color.min_element();
    let span = (max - min).max(1e-6);
    let degrees = match max {
        max if max == color.x => 60.0 * ((color.y - color.z) / span),
        max if max == color.y => 60.0 * ((color.z - color.x) / span + 2.0),
        _ => 60.0 * ((color.x - color.y) / span + 4.0),
    };
    degrees.rem_euclid(360.0)
}

fn decoded(found: [u8; 3]) -> Vec3 {
    Vec3::from_array(found.map(crate::look::canvas::decode))
}

#[test]
fn hue_preserving_tone_mapping_keeps_the_hue_of_bright_colours() {
    let mut canvas = Canvas::new();
    let color = Vec3::new(4.0, 0.6, 0.15);
    let shifted = mapped(&mut canvas, color, Vec::new(), Tonemap::new(Tone::Hable));
    let kept = mapped(
        &mut canvas,
        color,
        Vec::new(),
        Tonemap {
            preserve_hue: 1.0,
            ..Tonemap::new(Tone::Hable)
        },
    );
    let source = hue(color);
    assert!(
        (hue(decoded(shifted)) - source).abs() > 5.0,
        "per-channel mapping already keeps the hue: {shifted:?}"
    );
    assert!(
        (hue(decoded(kept)) - source).abs() < 2.0,
        "hue {} instead of {source}: {kept:?}",
        hue(decoded(kept))
    );
    assert!(
        kept[0] >= 250,
        "the bright colour lost its brightness: {kept:?}"
    );
}

fn to_acescct(value: f32) -> f32 {
    match value <= 0.007_812_5 {
        true => 10.540_237 * value + 0.072_905_53,
        false => (value.log2() + 9.72) / 17.52,
    }
}

fn from_acescct(value: f32) -> f32 {
    match value <= 0.155_251_14 {
        true => (value - 0.072_905_53) / 10.540_237,
        false => (value * 17.52 - 9.72).exp2(),
    }
}

#[test]
fn cdl_applies_slope_offset_power_and_saturation() {
    let mut canvas = Canvas::new();
    let cdl = canvas.effect(BuiltinEffect::Cdl);
    let input = Vec3::new(0.4, 0.3, 0.2);
    let slope = Vec3::new(1.2, 1.0, 0.8);
    let offset = Vec3::new(0.02, 0.0, -0.01);
    let power = Vec3::new(1.1, 1.0, 0.9);
    let saturation = 0.8;
    let decide = |value: Vec3| {
        let graded = (value * slope + offset).max(Vec3::ZERO).powf_each(power);
        let luma = graded.dot(Vec3::new(0.2126, 0.7152, 0.0722));
        Vec3::splat(luma) + saturation * (graded - Vec3::splat(luma))
    };
    for space in ["linear", "acescct"] {
        let instance = EffectInstance::new(cdl)
            .set("slope", Value::Vec3(slope))
            .set("offset", Value::Vec3(offset))
            .set("power", Value::Vec3(power))
            .set("saturation", saturation)
            .set("space", Value::Enum(space.into()));
        let expected = match space {
            "linear" => decide(input),
            _ => decide(input.to_array().map(to_acescct).into())
                .to_array()
                .map(from_acescct)
                .into(),
        };
        let found = mapped(
            &mut canvas,
            input,
            vec![instance.into()],
            Tonemap::new(Tone::Clamp),
        );
        assert!(
            close(found, expected, 1.5),
            "{space}: {found:?} instead of {expected:?}"
        );
    }
}

trait PowEach {
    fn powf_each(self, power: Vec3) -> Vec3;
}

impl PowEach for Vec3 {
    fn powf_each(self, power: Vec3) -> Vec3 {
        Vec3::new(
            self.x.powf(power.x),
            self.y.powf(power.y),
            self.z.powf(power.z),
        )
    }
}

const RGB_TO_XYZ: [[f32; 3]; 3] = [
    [0.412_456_4, 0.357_576_1, 0.180_437_5],
    [0.212_672_9, 0.715_152_2, 0.072_175],
    [0.019_333_9, 0.119_192, 0.950_304_1],
];
const BRADFORD: [[f32; 3]; 3] = [
    [0.8951, 0.2664, -0.1614],
    [-0.7502, 1.7135, 0.0367],
    [0.0389, -0.0685, 1.0296],
];

fn rows(values: [[f32; 3]; 3]) -> Mat3 {
    Mat3::from_cols_array_2d(&values).transpose()
}

fn planckian(kelvin: f32) -> Vec2 {
    let t = kelvin as f64;
    let u = (0.860_117_757 + 1.541_182_54e-4 * t + 1.286_412_12e-7 * t * t)
        / (1.0 + 8.424_202_35e-4 * t + 7.081_451_63e-7 * t * t);
    let v = (0.317_398_726 + 4.228_062_45e-5 * t + 4.204_816_91e-8 * t * t)
        / (1.0 - 2.897_418_16e-5 * t + 1.614_560_53e-7 * t * t);
    Vec2::new(u as f32, v as f32)
}

fn uv_of(xy: Vec2) -> Vec2 {
    let denominator = -2.0 * xy.x + 12.0 * xy.y + 3.0;
    Vec2::new(4.0 * xy.x, 6.0 * xy.y) / denominator
}

fn white_of(uv: Vec2) -> Vec3 {
    let denominator = 2.0 * uv.x - 8.0 * uv.y + 4.0;
    let (x, y) = (3.0 * uv.x / denominator, 2.0 * uv.y / denominator);
    Vec3::new(x / y, 1.0, (1.0 - x - y) / y)
}

#[test]
fn white_balance_neutralizes_its_light_with_a_bradford_adaptation() {
    let mut canvas = Canvas::new();
    let balance = canvas.effect(BuiltinEffect::WhiteBalance);
    let (kelvin, tint) = (3200.0, 0.01);
    let d65 = uv_of(Vec2::new(0.31271, 0.32902));
    let here = planckian(kelvin);
    let along = (planckian(kelvin + 1.0) - here).normalize();
    let mut normal = Vec2::new(-along.y, along.x);
    if normal.y < 0.0 {
        normal = -normal;
    }
    let light = white_of(here + normal * tint + (d65 - planckian(6504.0)));
    let to_xyz = rows(RGB_TO_XYZ);
    let bradford = rows(BRADFORD);
    let adapt = to_xyz.inverse()
        * bradford.inverse()
        * Mat3::from_diagonal(bradford * white_of(d65) / (bradford * light))
        * bradford
        * to_xyz;
    let instance = EffectInstance::new(balance)
        .set("temperature", kelvin)
        .set("tint", tint);
    let lit = to_xyz.inverse() * light * 0.25;
    for (input, expected) in [
        (Vec3::new(0.5, 0.4, 0.3), adapt * Vec3::new(0.5, 0.4, 0.3)),
        (lit, Vec3::splat(0.25)),
    ] {
        let found = mapped(
            &mut canvas,
            input,
            vec![instance.clone().into()],
            Tonemap::new(Tone::Clamp),
        );
        assert!(
            close(found, expected, 2.0),
            "{input:?}: {found:?} instead of {expected:?}"
        );
    }
}

#[test]
fn physical_exposure_follows_the_ev100_formula() {
    let mut canvas = Canvas::new();
    let (aperture, shutter, iso, compensation) = (2.8_f32, 1.0 / 50.0, 400.0_f32, 0.5_f32);
    let ev100 = (aperture * aperture / shutter).log2() - (iso / 100.0).log2();
    let exposure = NITS_PER_RADIANCE_UNIT * compensation.exp2() / (1.2 * ev100.exp2());
    let radiance = 0.4 / exposure;
    let physical = Exposure {
        metering: Metering::Physical,
        aperture,
        shutter,
        iso,
        compensation,
        ..Exposure::default()
    };
    let found = mapped(
        &mut canvas,
        Vec3::splat(radiance),
        vec![physical.into()],
        Tonemap::new(Tone::Clamp),
    );
    assert!(
        close(found, Vec3::splat(0.4), 1.5),
        "{found:?} instead of {}",
        encode(0.4)
    );
}
