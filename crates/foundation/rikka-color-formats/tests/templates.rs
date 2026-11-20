use rikka_color_formats::{NEW_FORMAT, Rgb, Template, template_named};
use rikka_settings::session::color_picker::built_in_formats;

#[test]
fn template_parses_like_powertoys() {
    let moccasin = Rgb::MOCCASIN;
    for (source, written) in [
        ("", "ffe4b5"),
        ("#%ReX%GrX%BlX", "#FFE4B5"),
        ("rgba(%Re, %Gr, %Bl, %Al)", "rgba(255, 228, 181, 255)"),
        (
            "Color.FromArgb(%Re, %Gr, %Bl)",
            "Color.FromArgb(255, 228, 181)",
        ),
        ("%Reh%Grh%Blh", "feb"),
        ("%ReH%GrH%BlH", "FEB"),
        ("%Ref %Grf %Blf", "1 0.89 0.71"),
        ("%ReF %GrF %BlF", "1 .89 .71"),
        ("%Reb%Gr", "255228"),
        ("%Res", "255"),
        ("%Hus", "38"),
        ("%%Re", "%255"),
        ("100%", "100%"),
        ("%R", "%R"),
        ("%Zz%Re", "%Zz255"),
        ("%Re%", "255%"),
        ("%Rez", "255z"),
        ("%Dr / %Dv", "16770229 / 11920639"),
        (NEW_FORMAT, "new Color (R = 255, G = 228, B = 181)"),
    ] {
        assert_eq!(
            Template::parse(source).render(moccasin),
            written,
            "{source:?}"
        );
    }
}

#[test]
fn name_parameter_renders_colour_name() {
    let red = Rgb::new(255, 0, 0);
    let named = Template::parse("%Na: %ReX%Na");
    assert_eq!(
        named.render(red),
        format!("{0}: FF{0}", red.name().label().as_str())
    );
}

#[test]
fn copied_format_falls_back_to_hex() {
    let formats = built_in_formats();
    let orange = Rgb::new(255, 128, 0);
    assert_eq!(
        template_named(&formats, "RGB").render(orange),
        "rgb(255, 128, 0)"
    );
    assert_eq!(template_named(&formats, "missing").render(orange), "ff8000");
    assert_eq!(template_named(&[], "HEX").render(orange), "ff8000");
}
