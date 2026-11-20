use material_colors::color::Argb;
use material_colors::dynamic_color::{DynamicScheme, Variant as OracleVariant};
use material_colors::quantize::{Quantizer, QuantizerCelebi};
use material_colors::score::Score;
use rikka_material_color::{Hct, Rgb, Scheme, TonalPalette, Variant, harmonize, quantize, score};

const VARIANTS: [(Variant, OracleVariant); 9] = [
    (Variant::TonalSpot, OracleVariant::TonalSpot),
    (Variant::Vibrant, OracleVariant::Vibrant),
    (Variant::Expressive, OracleVariant::Expressive),
    (Variant::Neutral, OracleVariant::Neutral),
    (Variant::Monochrome, OracleVariant::Monochrome),
    (Variant::Fidelity, OracleVariant::Fidelity),
    (Variant::Content, OracleVariant::Content),
    (Variant::Rainbow, OracleVariant::Rainbow),
    (Variant::FruitSalad, OracleVariant::FruitSalad),
];

const SOURCES: [u32; 16] = [
    0x000000, 0xFFFFFF, 0x808080, 0xFF0000, 0x00FF00, 0x0000FF, 0x4285F4, 0x6750A4, 0xB3261E,
    0xFFDE3F, 0x00696B, 0x8E4585, 0x1B1B1F, 0xF2B8B5, 0x7D5260, 0x0B57D0,
];

fn rgb(hex: u32) -> Rgb {
    Rgb::new((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

fn argb(color: Rgb) -> Argb {
    Argb::new(255, color.red, color.green, color.blue)
}

fn plain(color: Argb) -> Rgb {
    Rgb::new(color.red, color.green, color.blue)
}

struct Random(u64);

impl Random {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
}

#[test]
fn measuring_a_colour_matches_the_reference_exactly() {
    for red in (0..=255u8).step_by(17) {
        for green in (0..=255u8).step_by(17) {
            for blue in (0..=255u8).step_by(17) {
                let color = Rgb::new(red, green, blue);
                let ours = Hct::of(color);
                let theirs = material_colors::hct::Hct::new(argb(color));
                assert_eq!(
                    (ours.hue(), ours.chroma(), ours.tone()),
                    (theirs.get_hue(), theirs.get_chroma(), theirs.get_tone()),
                    "{color:?}"
                );
                assert_eq!(color.lstar(), argb(color).as_lstar(), "{color:?}");
            }
        }
    }
}

#[test]
fn solving_hue_chroma_and_tone_matches_the_reference() {
    for hue in (0..360).step_by(7) {
        for chroma in [0.0, 2.0, 8.0, 16.0, 36.0, 48.0, 84.0, 120.0, 200.0] {
            for tone in (0..=100).step_by(4) {
                let (hue, tone) = (f64::from(hue) + 0.25, f64::from(tone));
                let ours = Hct::solve(hue, chroma, tone);
                let theirs = material_colors::hct::Hct::from(hue, chroma, tone);
                assert_eq!(
                    ours.rgb(),
                    plain(theirs.into()),
                    "hue {hue} chroma {chroma} tone {tone}"
                );
                assert_eq!(
                    TonalPalette::new(hue, chroma).tone(tone),
                    ours.rgb(),
                    "hue {hue} chroma {chroma} tone {tone}"
                );
            }
        }
    }
}

#[test]
fn harmonizing_matches_the_reference() {
    for design in SOURCES {
        for source in SOURCES {
            let ours = harmonize(rgb(design), rgb(source));
            let theirs = material_colors::blend::harmonize(argb(rgb(design)), argb(rgb(source)));
            assert_eq!(ours, plain(theirs), "{design:06X} toward {source:06X}");
        }
    }
}

#[test]
fn every_scheme_role_matches_the_reference_at_every_contrast() {
    for source in SOURCES {
        for (variant, oracle_variant) in VARIANTS {
            for dark in [false, true] {
                for contrast in [-1.0, -0.5, 0.0, 0.25, 0.5, 0.8, 1.0] {
                    let ours = Scheme::new(rgb(source), variant, dark, contrast);
                    let theirs: material_colors::scheme::Scheme = DynamicScheme::by_variant(
                        argb(rgb(source)),
                        &oracle_variant,
                        dark,
                        Some(contrast),
                    )
                    .into();
                    let expected = Scheme {
                        primary: plain(theirs.primary),
                        error: plain(theirs.error),
                        surface: plain(theirs.surface),
                        surface_container_lowest: plain(theirs.surface_container_lowest),
                        surface_container_low: plain(theirs.surface_container_low),
                        surface_container: plain(theirs.surface_container),
                        surface_container_high: plain(theirs.surface_container_high),
                        surface_container_highest: plain(theirs.surface_container_highest),
                        on_surface: plain(theirs.on_surface),
                        on_surface_variant: plain(theirs.on_surface_variant),
                        outline: plain(theirs.outline),
                        outline_variant: plain(theirs.outline_variant),
                    };
                    assert_eq!(
                        ours, expected,
                        "{source:06X} {variant:?} dark {dark} contrast {contrast}"
                    );
                }
            }
        }
    }
}

fn pictures() -> Vec<(&'static str, Vec<Rgb>)> {
    let mut random = Random(5);
    let side = 128usize;
    let gradient = (0..side * side)
        .map(|index| {
            let (x, y) = (index % side, index / side);
            Rgb::new((x * 2) as u8, (y * 2) as u8, ((x + y) % 256) as u8)
        })
        .collect();
    let noise = (0..side * side)
        .map(|_| {
            let value = random.next();
            Rgb::new(value as u8, (value >> 8) as u8, (value >> 16) as u8)
        })
        .collect();
    let poster = (0..side * side)
        .map(|index| rgb(SOURCES[(index / 97 + index % 5) % SOURCES.len()]))
        .collect();
    let sunset = (0..side * side)
        .map(|index| {
            let (x, y) = (index % side, index / side);
            let wobble = (random.next() % 12) as u8;
            Rgb::new(
                200u8.saturating_add((y / 3) as u8).saturating_sub(wobble),
                (60 + y) as u8,
                (40 + x / 4) as u8 + wobble,
            )
        })
        .collect();
    vec![
        ("gradient", gradient),
        ("noise", noise),
        ("poster", poster),
        ("sunset", sunset),
        ("flat", vec![Rgb::new(12, 200, 90); 400]),
        (
            "pair",
            [Rgb::new(250, 10, 10), Rgb::new(10, 10, 250)].repeat(300),
        ),
    ]
}

#[test]
fn quantizing_and_scoring_a_picture_matches_the_reference() {
    for (name, pixels) in pictures() {
        let reference: Vec<Argb> = pixels.iter().map(|&pixel| argb(pixel)).collect();
        let theirs = QuantizerCelebi::quantize(&reference, 128);
        let expected: Vec<(Rgb, u32)> = theirs
            .color_to_count
            .iter()
            .map(|(&color, &count)| (plain(color), count))
            .collect();

        let ours = quantize(&pixels, 128);

        assert_eq!(ours, expected, "{name}");
        let ranked: Vec<Rgb> = Score::score(&theirs.color_to_count, None, None, None)
            .into_iter()
            .map(plain)
            .collect();
        assert_eq!(score(&ours), ranked, "{name}");
    }
    assert!(quantize(&[], 128).is_empty());
    assert_eq!(score(&[]), [Rgb::new(66, 133, 244)]);
}
