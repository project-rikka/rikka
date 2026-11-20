use rikka_effects::{ColorLookup, Effect, Kind};
use rikka_image_edit::{GaussianBlur, Operation};
use serde_json::json;

fn fresh() -> impl Iterator<Item = Effect> {
    Kind::ALL.iter().map(|kind| kind.default_effect())
}

fn loaded(value: serde_json::Value) -> Effect {
    serde_json::from_value::<Effect>(value)
        .expect("effect parses")
        .checked()
}
#[test]
fn knob_set_clamps_and_updates_recipe() {
    for effect in fresh() {
        for knob in effect.knobs() {
            let mut changed = effect.clone();
            assert!(changed.set(knob.key, f32::MAX));
            let most = changed.value(knob.key).expect("value");
            assert!(
                most <= *knob.scale.range.end(),
                "{} reached {most}",
                knob.key
            );
            assert!(changed.set(knob.key, f32::MIN));
            let least = changed.value(knob.key).expect("value");
            assert!(
                least >= *knob.scale.range.start(),
                "{} reached {least}",
                knob.key
            );
        }
    }
}

#[test]
fn rejects_unknown_keys_and_invalid_values() {
    let mut blur = Kind::GaussianBlur.default_effect();

    assert!(!blur.set("sigma", 3.0));
    assert!(!blur.set("radius", f32::NAN));
    assert_eq!(blur.value("sigma"), None);
    assert_eq!(blur.value("radius"), Some(1.0));
}

#[test]
fn knob_targets_named_operation() {
    let mut blur = Kind::GaussianBlur.default_effect();
    blur.set("radius", 3.0);

    let recipe = blur.recipe();
    assert_eq!(
        recipe.steps()[0].operation(),
        &Operation::from(GaussianBlur::new(3.0))
    );
    assert_eq!(blur.padding(), 9.0);
    assert_eq!(Kind::Levels.default_effect().padding(), 0.0);
}
#[test]
fn effects_round_trip_through_serde() {
    for effect in fresh() {
        let mut changed = effect.clone();
        if let Some(knob) = changed.knobs().first() {
            changed.set(knob.key, *knob.scale.range.end());
        }
        let serialized = serde_json::to_value(&changed).expect("effect serializes");
        let read: Effect = serde_json::from_value(serialized).expect("effect parses");
        assert_eq!(read, changed);
    }
}

#[test]
fn effects_serialize_snake_case_names() {
    let mut blur = Kind::GaussianBlur.default_effect();
    blur.set("radius", 4.0);
    assert_eq!(
        serde_json::to_value(&blur).expect("json"),
        json!({ "kind": "gaussian_blur", "radius": 4.0 })
    );

    let hue = serde_json::to_value(Kind::HueSaturation.default_effect()).expect("json");
    assert_eq!(hue["kind"], "hue_saturation");
    assert_eq!(hue["colorized"]["saturation"], 25.0);
    assert_eq!(
        hue["reds"],
        json!({ "hue": 0.0, "saturation": 0.0, "lightness": 0.0 })
    );

    let filter = serde_json::to_value(Kind::PhotoFilter.default_effect()).expect("json");
    assert_eq!(filter["filter"], "warming85");
    assert_eq!(filter["color"], "#ec8a00");
}

#[test]
fn missing_fields_use_defaults() {
    assert_eq!(
        loaded(json!({ "kind": "glow" })),
        Kind::Glow.default_effect()
    );

    let vignette = loaded(json!({ "kind": "vignette", "amount": 20.0, "grain": 3 }));
    let mut expected = Kind::Vignette.default_effect();
    expected.set("amount", 20.0);
    assert_eq!(vignette, expected);

    let hue = loaded(json!({ "kind": "hue_saturation", "greens": { "hue": 30.0 } }));
    assert_eq!(hue.value("greens.hue"), Some(30.0));
    assert_eq!(hue.value("greens.saturation"), Some(0.0));
    assert_eq!(hue.value("colorized.saturation"), Some(25.0));
}

#[test]
fn validate_clamps_loaded_values() {
    let blur = loaded(json!({ "kind": "gaussian_blur", "radius": 5000.0 }));
    assert_eq!(blur.value("radius"), Some(1000.0));

    let levels = loaded(json!({ "kind": "levels", "red": { "black": 240.0, "white": 30.0 } }));
    let (black, white) = (
        levels.value("red.black").expect("black"),
        levels.value("red.white").expect("white"),
    );
    assert!(black + 2.0 <= white, "{black} {white}");

    let curves = loaded(json!({ "kind": "curves", "red": [[0.0, 0.0], [2.0, 1.0], [0.5, 0.8]] }));
    let serialized = serde_json::to_value(&curves).expect("json");
    assert_eq!(
        serialized["red"],
        json!([[0.0, 0.0], [0.5, 0.800000011920929], [1.0, 1.0]])
    );
    assert_eq!(serialized["green"], json!([[0.0, 0.0], [1.0, 1.0]]));
}

#[test]
fn gradient_map_keeps_two_stops() {
    let map = loaded(json!({
        "kind": "gradient_map",
        "stops": [{ "at": 0.4, "color": "#ff0000" }],
    }));
    assert_eq!(map, Kind::GradientMap.default_effect());

    let sepia = loaded(json!({
        "kind": "gradient_map",
        "stops": [
            { "at": 1.0, "color": "#ffffff" },
            { "at": 0.0, "color": "#000000", "midpoint": 0.3 },
        ],
        "reversed": true,
    }));
    let serialized = serde_json::to_value(&sepia).expect("json");
    assert_eq!(serialized["stops"][0]["color"], "#000000");
    assert!(
        (serialized["stops"][0]["midpoint"]
            .as_f64()
            .expect("midpoint")
            - 0.3)
            .abs()
            < 1e-6
    );
    assert_eq!(serialized["reversed"], true);
}

#[test]
fn lookup_table_serializes_with_effect() {
    let entries: String = (0..8)
        .map(|at| format!("{} {} {}\n", at & 1, (at >> 1) & 1, (at >> 2) & 1))
        .collect();
    let source = format!("LUT_3D_SIZE 2\n{entries}");
    let lookup = Effect::from(ColorLookup::parse("identity.cube", &source).expect("table"));

    let serialized = serde_json::to_value(&lookup).expect("json");
    assert_eq!(serialized["name"], "identity.cube");
    let read: Effect = serde_json::from_value(serialized).expect("effect parses");
    assert_eq!(read, lookup);
    assert_eq!(read.recipe().steps().len(), 1);

    assert!(ColorLookup::parse("broken.cube", "LUT_3D_SIZE 2\n0 0 0\n").is_err());
    let broken = loaded(json!({ "kind": "color_lookup", "name": "broken", "source": "nonsense" }));
    assert!(broken.recipe().is_empty());
}

#[test]
fn knob_keys_are_stable() {
    let expected: [(Kind, &[&str]); 43] = [
        (Kind::BrightnessContrast, &["brightness", "contrast"]),
        (
            Kind::Levels,
            &[
                "rgb.black",
                "rgb.gamma",
                "rgb.white",
                "rgb.out_black",
                "rgb.out_white",
                "red.black",
                "red.gamma",
                "red.white",
                "red.out_black",
                "red.out_white",
                "green.black",
                "green.gamma",
                "green.white",
                "green.out_black",
                "green.out_white",
                "blue.black",
                "blue.gamma",
                "blue.white",
                "blue.out_black",
                "blue.out_white",
            ],
        ),
        (Kind::Curves, &[]),
        (Kind::Exposure, &["exposure", "offset", "gamma"]),
        (Kind::Vibrance, &["vibrance", "saturation"]),
        (
            Kind::HueSaturation,
            &[
                "master.hue",
                "master.saturation",
                "master.lightness",
                "reds.hue",
                "reds.saturation",
                "reds.lightness",
                "yellows.hue",
                "yellows.saturation",
                "yellows.lightness",
                "greens.hue",
                "greens.saturation",
                "greens.lightness",
                "cyans.hue",
                "cyans.saturation",
                "cyans.lightness",
                "blues.hue",
                "blues.saturation",
                "blues.lightness",
                "magentas.hue",
                "magentas.saturation",
                "magentas.lightness",
                "colorized.hue",
                "colorized.saturation",
                "colorized.lightness",
            ],
        ),
        (
            Kind::ColorBalance,
            &[
                "shadows.cyan_red",
                "shadows.magenta_green",
                "shadows.yellow_blue",
                "midtones.cyan_red",
                "midtones.magenta_green",
                "midtones.yellow_blue",
                "highlights.cyan_red",
                "highlights.magenta_green",
                "highlights.yellow_blue",
            ],
        ),
        (
            Kind::BlackWhite,
            &["reds", "yellows", "greens", "cyans", "blues", "magentas"],
        ),
        (Kind::PhotoFilter, &["density"]),
        (
            Kind::ChannelMixer,
            &[
                "red.red",
                "red.green",
                "red.blue",
                "red.constant",
                "green.red",
                "green.green",
                "green.blue",
                "green.constant",
                "blue.red",
                "blue.green",
                "blue.blue",
                "blue.constant",
                "gray.red",
                "gray.green",
                "gray.blue",
                "gray.constant",
            ],
        ),
        (Kind::ColorLookup, &[]),
        (Kind::WhiteBalance, &["temperature", "tint"]),
        (Kind::Posterize, &["levels"]),
        (Kind::Threshold, &["level"]),
        (Kind::GradientMap, &[]),
        (
            Kind::SelectiveColor,
            &[
                "reds.cyan",
                "reds.magenta",
                "reds.yellow",
                "reds.black",
                "yellows.cyan",
                "yellows.magenta",
                "yellows.yellow",
                "yellows.black",
                "greens.cyan",
                "greens.magenta",
                "greens.yellow",
                "greens.black",
                "cyans.cyan",
                "cyans.magenta",
                "cyans.yellow",
                "cyans.black",
                "blues.cyan",
                "blues.magenta",
                "blues.yellow",
                "blues.black",
                "magentas.cyan",
                "magentas.magenta",
                "magentas.yellow",
                "magentas.black",
                "whites.cyan",
                "whites.magenta",
                "whites.yellow",
                "whites.black",
                "neutrals.cyan",
                "neutrals.magenta",
                "neutrals.yellow",
                "neutrals.black",
                "blacks.cyan",
                "blacks.magenta",
                "blacks.yellow",
                "blacks.black",
            ],
        ),
        (
            Kind::ShadowsHighlights,
            &[
                "shadows",
                "shadow_tone",
                "highlights",
                "highlight_tone",
                "radius",
                "color",
                "midtones",
            ],
        ),
        (Kind::GaussianBlur, &["radius"]),
        (Kind::BoxBlur, &["radius"]),
        (Kind::MotionBlur, &["angle", "distance"]),
        (
            Kind::RadialBlur,
            &["spin", "amount", "center_x", "center_y"],
        ),
        (
            Kind::LensBlur,
            &["radius", "curvature", "rotation", "brightness", "threshold"],
        ),
        (Kind::SurfaceBlur, &["radius", "threshold"]),
        (Kind::Median, &["radius"]),
        (
            Kind::TiltShift,
            &["blur", "center", "angle", "focus", "transition"],
        ),
        (Kind::UnsharpMask, &["amount", "radius", "threshold"]),
        (
            Kind::SmartSharpen,
            &["amount", "radius", "reduce_noise", "angle"],
        ),
        (Kind::HighPass, &["radius"]),
        (Kind::Clarity, &["amount", "radius"]),
        (Kind::AddNoise, &["amount", "seed"]),
        (
            Kind::ReduceNoise,
            &["strength", "preserve_details", "color", "sharpen_details"],
        ),
        (Kind::Emboss, &["angle", "height", "amount"]),
        (Kind::Mosaic, &["cell"]),
        (
            Kind::OilPaint,
            &[
                "stylization",
                "cleanliness",
                "scale",
                "bristle_detail",
                "angle",
                "shine",
            ],
        ),
        (Kind::Glow, &["radius", "brightness", "threshold"]),
        (
            Kind::Vignette,
            &["amount", "midpoint", "roundness", "feather", "highlights"],
        ),
        (Kind::Dither, &["levels"]),
        (Kind::ChromaKey, &["tolerance", "softness", "spill"]),
        (Kind::LumaKey, &["threshold", "softness"]),
        (
            Kind::Clipping,
            &["left", "top", "right", "bottom", "feather"],
        ),
        (Kind::ChannelShift, &["angle", "distance"]),
        (Kind::DropShadow, &["angle", "distance", "blur", "opacity"]),
        (Kind::Outline, &["width", "softness"]),
    ];
    assert_eq!(expected.map(|(kind, _)| kind).as_slice(), Kind::ALL);
    for (kind, keys) in expected {
        let effect = kind.default_effect();
        let named: Vec<&str> = effect.knobs().iter().map(|knob| knob.key).collect();
        assert_eq!(named, keys, "{kind:?}");
        for key in keys {
            assert!(effect.value(key).is_some(), "{kind:?} cannot read {key}");
        }
    }
}
