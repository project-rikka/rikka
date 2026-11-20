use rikka_color_formats::{Rgb, Template};
use rikka_settings::session::color_picker::built_in_formats;

fn hex(typed: &str) -> Rgb {
    Rgb::parse_hex(typed).expect("a hex colour")
}

fn rounded(value: f64, digits: i32) -> f64 {
    let power = 10f64.powi(digits);
    (value * power).round_ties_even() / power
}

fn built_in(name: &str) -> Template {
    let format = built_in_formats()
        .into_iter()
        .find(|format| format.name == name)
        .expect("a built-in format");
    Template::parse(&format.format)
}

#[test]
fn builtin_formats_match_powertoys() {
    let black = Rgb::new(0, 0, 0);
    let red = Rgb::new(255, 0, 0);
    let white = Rgb::new(255, 255, 255);
    let green = Rgb::new(0, 255, 0);
    let blue = Rgb::new(0, 0, 255);
    for (rgb, name, written) in [
        (black, "CMYK", "cmyk(0%, 0%, 0%, 100%)"),
        (black, "HEX", "000000"),
        (black, "NCol", "R0, 0%, 100%"),
        (black, "HSB", "hsb(0, 0%, 0%)"),
        (black, "HSI", "hsi(0, 0%, 0%)"),
        (black, "HSL", "hsl(0, 0%, 0%)"),
        (black, "HSV", "hsv(0, 0%, 0%)"),
        (black, "HWB", "hwb(0, 0%, 100%)"),
        (black, "RGB", "rgb(0, 0, 0)"),
        (black, "CIEXYZ", "XYZ(0, 0, 0)"),
        (black, "CIELAB", "CIELab(0, 0, 0)"),
        (black, "Oklab", "oklab(0, 0, 0)"),
        (black, "Oklch", "oklch(0, 0, 0)"),
        (black, "VEC4", "(0f, 0f, 0f, 1f)"),
        (black, "Decimal", "0"),
        (black, "HEX Int", "0xFF000000"),
        (red, "CMYK", "cmyk(0%, 100%, 100%, 0%)"),
        (red, "HEX", "ff0000"),
        (red, "RGB", "rgb(255, 0, 0)"),
        (red, "HSL", "hsl(0, 100%, 50%)"),
        (red, "HSV", "hsv(0, 100%, 100%)"),
        (red, "HSB", "hsb(0, 100%, 100%)"),
        (red, "HSI", "hsi(0, 100%, 33%)"),
        (red, "HWB", "hwb(0, 0%, 0%)"),
        (red, "Decimal", "255"),
        (red, "HEX Int", "0xFFFF0000"),
        (red, "VEC4", "(1f, 0f, 0f, 1f)"),
        (white, "CMYK", "cmyk(0%, 0%, 0%, 0%)"),
        (white, "HEX", "ffffff"),
        (white, "RGB", "rgb(255, 255, 255)"),
        (white, "HSL", "hsl(0, 0%, 100%)"),
        (white, "HSV", "hsv(0, 0%, 100%)"),
        (white, "Decimal", "16777215"),
        (white, "HEX Int", "0xFFFFFFFF"),
        (white, "VEC4", "(1f, 1f, 1f, 1f)"),
        (green, "CMYK", "cmyk(100%, 0%, 100%, 0%)"),
        (green, "HEX", "00ff00"),
        (green, "HSL", "hsl(120, 100%, 50%)"),
        (green, "Decimal", "65280"),
        (green, "HEX Int", "0xFF00FF00"),
        (blue, "CMYK", "cmyk(100%, 100%, 0%, 0%)"),
        (blue, "HSV", "hsv(240, 100%, 100%)"),
        (blue, "Decimal", "16711680"),
        (blue, "HEX Int", "0xFF0000FF"),
    ] {
        assert_eq!(built_in(name).render(rgb), written, "{name} of {rgb}");
    }
}

#[test]
fn cielab_matches_reference() {
    for (typed, lightness, a, b) in [
        ("FFFFFF", 100.00, 0.00, 0.00),
        ("808080", 53.59, 0.00, 0.00),
        ("FF0000", 53.24, 80.09, 67.20),
        ("008000", 46.23, -51.70, 49.90),
        ("80FFFF", 93.16, -35.23, -10.87),
        ("8080FF", 59.20, 33.10, -63.46),
        ("BF40BF", 50.10, 65.50, -41.48),
        ("BFBF00", 75.04, -17.35, 76.03),
        ("0048BA", 34.35, 27.94, -64.80),
        ("1A2421", 13.18, -5.23, 0.56),
        ("EFDFBB", 89.26, -0.13, 19.65),
        ("5218FA", 36.65, 75.63, -97.70),
        ("E3F988", 94.25, -23.70, 51.58),
    ] {
        let lab = hex(typed).lab();
        assert_eq!(
            [lab.lightness, lab.a, lab.b].map(|value| rounded(value, 2) + 0.0),
            [lightness, a, b],
            "{typed}"
        );
    }
    assert_eq!(
        built_in("CIELAB").render(hex("FF0000")),
        "CIELab(53.24, 80.09, 67.2)"
    );
    assert_eq!(
        Template::parse("%Lci, %Cai, %Cbi").render(hex("8080FF")),
        "59, 33, -63"
    );
}

#[test]
fn oklab_oklch_match_reference() {
    for (typed, lightness, chroma, hue) in [
        ("FFFFFF", 1.00, 0.00, 0.00),
        ("808080", 0.6, 0.00, 0.00),
        ("FF0000", 0.628, 0.258, 29.23),
        ("008000", 0.52, 0.177, 142.5),
        ("8080FF", 0.661, 0.184, 280.13),
        ("BF40BF", 0.598, 0.216, 327.86),
        ("D70A53", 0.563, 0.222, 11.5),
        ("E3F988", 0.942, 0.141, 118.24),
    ] {
        let oklch = hex(typed).oklch();
        assert_eq!(
            [
                rounded(oklch.lightness, 3),
                rounded(oklch.chroma, 3),
                rounded(oklch.hue, 2)
            ],
            [lightness, chroma, hue],
            "{typed}"
        );
    }
    let oklab = hex("FF0000").oklab();
    assert_eq!(
        [rounded(oklab.a, 2), rounded(oklab.b, 2)],
        [0.22, 0.13],
        "the Oklab axes of red"
    );
}

#[test]
fn natural_hues_match_reference() {
    for (typed, natural) in [
        ("FFFFFF", "R0"),
        ("BFBF00", "Y0"),
        ("008000", "G0"),
        ("80FFFF", "C0"),
        ("8080FF", "B0"),
        ("BF40BF", "M0"),
        ("A0A424", "Y3"),
        ("411BEA", "B18"),
        ("1EAC41", "G25"),
        ("F0C80E", "R82"),
        ("B430E5", "B73"),
        ("ED7651", "R24"),
        ("FEF888", "R95"),
        ("19CB97", "G71"),
        ("362698", "B14"),
    ] {
        assert_eq!(
            Template::parse("%Hn").render(hex(typed)),
            natural,
            "{typed}"
        );
    }
}

#[test]
fn parses_hex_variants() {
    for (typed, read) in [
        ("#ff8000", Some(Rgb::new(255, 128, 0))),
        ("FF8000", Some(Rgb::new(255, 128, 0))),
        (" #f80 ", Some(Rgb::new(255, 136, 0))),
        ("abc", Some(Rgb::new(170, 187, 204))),
        ("#ff80", None),
        ("#gg8000", None),
        ("", None),
    ] {
        assert_eq!(Rgb::parse_hex(typed), read, "{typed:?}");
    }
    assert_eq!(Rgb::new(255, 128, 0).hex(), "ff8000");
}

#[test]
fn shades_match_powertoys() {
    let shades = Rgb::new(255, 128, 0).shades();
    assert_eq!(
        shades.all(),
        [
            Rgb::new(255, 162, 0),
            Rgb::new(255, 145, 0),
            Rgb::new(204, 102, 0),
            Rgb::new(178, 89, 0),
        ]
    );
}
