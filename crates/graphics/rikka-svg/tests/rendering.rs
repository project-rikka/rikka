use rikka_svg::{ColorScheme, Error, Loader, MAX_PIXELS, Raster, Svg};

const SQUARE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">
    <rect width="10" height="10" fill="#ff0000"/>
</svg>"##;

const WIDE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 10">
    <rect width="20" height="10" fill="#0000ff"/>
</svg>"##;

const TRANSLUCENT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4">
    <rect width="4" height="4" fill="#ff0000" fill-opacity="0.5"/>
</svg>"##;

fn pixel(raster: &Raster, x: u32, y: u32) -> [u8; 4] {
    let at = (y * raster.width() + x) as usize * 4;
    raster.pixels()[at..at + 4]
        .try_into()
        .expect("pixel has four bytes")
}

#[test]
fn renders_at_every_requested_size() {
    let svg = Svg::from_bytes(SQUARE.as_bytes()).expect("valid document");

    for side in [16, 64, 256] {
        let raster = svg.render(side, side).expect("raster rendered");
        assert_eq!((raster.width(), raster.height()), (side, side));
        assert_eq!(raster.pixels().len(), (side * side * 4) as usize);
        assert_eq!(pixel(&raster, side / 2, side / 2), [255, 0, 0, 255]);
    }
}

#[test]
fn preserves_aspect_ratio_in_other_box() {
    let raster = Svg::from_bytes(WIDE.as_bytes())
        .expect("valid document")
        .render(40, 40)
        .expect("raster rendered");

    assert_eq!(pixel(&raster, 20, 20), [0, 0, 255, 255], "square interior");
    assert_eq!(
        pixel(&raster, 20, 2)[3],
        0,
        "band above square is transparent"
    );
    assert_eq!(
        pixel(&raster, 20, 37)[3],
        0,
        "band below square is transparent"
    );
}

#[test]
fn returns_straight_alpha_pixels() {
    let raster = Svg::from_bytes(TRANSLUCENT.as_bytes())
        .expect("valid document")
        .render(8, 8)
        .expect("raster rendered");

    let [red, green, blue, alpha] = pixel(&raster, 4, 4);
    assert_eq!(
        (red, green, blue),
        (255, 0, 0),
        "red stays red at half alpha"
    );
    assert!(
        (126..=130).contains(&alpha),
        "half transparent, got {alpha}"
    );
}

#[test]
fn rejects_malformed_documents() {
    assert!(matches!(
        Svg::from_bytes(b"not markup at all"),
        Err(Error::Malformed(_))
    ));
    assert!(matches!(
        Svg::from_bytes(b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"0\" height=\"0\"/>"),
        Err(Error::NoSize)
    ));
}

#[test]
fn rejects_unrenderable_sizes_before_allocating() {
    let svg = Svg::from_bytes(SQUARE.as_bytes()).expect("valid document");

    assert!(matches!(
        svg.render(0, 32),
        Err(Error::InvalidSize {
            width: 0,
            height: 32
        })
    ));

    let side = (MAX_PIXELS as f64).sqrt() as u32 + 1;
    assert!(matches!(
        svg.render(side, side),
        Err(Error::TooManyPixels { .. })
    ));
}

#[test]
fn expands_internal_entities_within_bounds() {
    let styled = r##"<!DOCTYPE svg [
        <!ENTITY ns "http://www.w3.org/2000/svg">
        <!ENTITY paint "fill:#00ff00;">
    ]>
    <svg xmlns="&ns;" viewBox="0 0 4 4"><rect width="4" height="4" style="&paint;"/></svg>"##;
    let raster = Svg::from_bytes(styled.as_bytes())
        .expect("valid document")
        .render(4, 4)
        .expect("raster rendered");
    assert_eq!(pixel(&raster, 2, 2), [0, 255, 0, 255]);

    let mut exploding = String::from("<!DOCTYPE svg [<!ENTITY e0 \"0123456789abcdef\">");
    for level in 1..=7 {
        let inner = format!("&e{};", level - 1).repeat(16);
        exploding.push_str(&format!("<!ENTITY e{level} \"{inner}\">"));
    }
    exploding.push_str("]><svg xmlns=\"http://www.w3.org/2000/svg\" id=\"&e7;\"/>");
    assert!(matches!(
        Svg::from_bytes(exploding.as_bytes()),
        Err(Error::TooComplex)
    ));

    let recursive = r#"<!DOCTYPE svg [<!ENTITY a "&b;"><!ENTITY b "&a;">]>
    <svg xmlns="http://www.w3.org/2000/svg" id="&a;"/>"#;
    assert!(matches!(
        Svg::from_bytes(recursive.as_bytes()),
        Err(Error::TooComplex)
    ));
}

#[test]
fn resolves_custom_properties_and_current_color() {
    let document = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 4"
        style="--accent: #0000ff" color="#ff0000">
        <rect width="4" height="4" fill="var(--accent, #000)"/>
        <rect x="4" width="4" height="4" fill="currentcolor"/>
    </svg>"##;
    let raster = Svg::from_bytes(document.as_bytes())
        .expect("valid document")
        .render(8, 4)
        .expect("raster rendered");
    assert_eq!(pixel(&raster, 1, 1), [0, 0, 255, 255]);
    assert_eq!(pixel(&raster, 6, 1), [255, 0, 0, 255]);
}

#[test]
fn non_scaling_strokes_keep_their_width_in_host_pixels() {
    let document = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">
        <g transform="scale(4)">
            <path d="M0 0.5H2.5" stroke="#ff0000" stroke-width="0.5"/>
            <path d="M0 2H2.5" stroke="#0000ff" stroke-width="0.5" vector-effect="non-scaling-stroke"/>
        </g>
    </svg>"##;
    let raster = Svg::from_bytes(document.as_bytes())
        .expect("valid document")
        .render(100, 100)
        .expect("raster rendered");
    assert_eq!(
        pixel(&raster, 50, 11),
        [255, 0, 0, 255],
        "scaled stroke spans 20 px"
    );
    assert_eq!(
        pixel(&raster, 50, 80),
        [0, 0, 255, 255],
        "host stroke centre"
    );
    assert_eq!(pixel(&raster, 50, 84)[3], 0, "host stroke spans 5 px");
}

#[test]
fn media_queries_follow_the_render_size_and_color_scheme() {
    let document = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4">
        <style>
            rect { fill: #ff0000 }
            @media (max-width: 32px) { rect { fill: #00ff00 } }
            @media (prefers-color-scheme: dark) and (min-width: 33px) { rect { fill: #0000ff } }
        </style>
        <rect width="4" height="4"/>
    </svg>"##;
    let light = Svg::from_bytes(document.as_bytes()).expect("valid document");
    let dark = Loader::default()
        .color_scheme(ColorScheme::Dark)
        .from_bytes(document.as_bytes())
        .expect("valid document");
    for (svg, side, expected) in [
        (&light, 16, [0, 255, 0, 255]),
        (&light, 64, [255, 0, 0, 255]),
        (&light, 24, [0, 255, 0, 255]),
        (&dark, 64, [0, 0, 255, 255]),
        (&dark, 32, [0, 255, 0, 255]),
    ] {
        let raster = svg.render(side, side).expect("raster rendered");
        assert_eq!(pixel(&raster, side / 2, side / 2), expected, "{side} px");
    }
}

#[test]
fn documents_report_whether_they_follow_the_color_scheme() {
    const NESTED: &str = "PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSI0IiBoZWlnaHQ9IjQiPjxzdHlsZT5AbWVkaWEgKHByZWZlcnMtY29sb3Itc2NoZW1lOiBkYXJrKSB7IHJlY3QgeyBmaWxsOiAjMDAwMGZmIH0gfTwvc3R5bGU+PHJlY3Qgd2lkdGg9IjQiIGhlaWdodD0iNCIgZmlsbD0iI2ZmMDAwMCIvPjwvc3ZnPg==";
    let cases = [
        (
            "@media (prefers-color-scheme: dark) { rect { fill: #0000ff } }",
            "",
            true,
        ),
        (
            "@media not (prefers-color-scheme: light) { rect { fill: #0000ff } }",
            "",
            true,
        ),
        (
            "@media (max-width: 32px) { rect { fill: #0000ff } }",
            "",
            false,
        ),
        ("rect { fill: #0000ff }", "", false),
        (
            "",
            &format!(r#"<image width="4" height="4" href="data:image/svg+xml;base64,{NESTED}"/>"#),
            true,
        ),
    ];
    for (style, body, expected) in cases {
        let document = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4">
                <style>{style}</style><rect width="4" height="4"/>{body}
            </svg>"##
        );
        let svg = Svg::from_bytes(document.as_bytes()).expect("valid document");
        assert_eq!(svg.follows_color_scheme(), expected, "{style}{body}");
    }
}

#[test]
fn structural_selectors_and_sibling_combinators_style_static_documents() {
    let cases: [(&str, &[u32]); 9] = [
        ("g > rect:last-child", &[2]),
        ("rect:nth-child(2n+1)", &[0, 2]),
        ("rect:nth-last-child(2)", &[1]),
        ("rect:not(.plain, #third)", &[1]),
        (".plain ~ rect", &[1, 2]),
        ("rect + rect + rect", &[2]),
        ("[data-role|=\"accent\"]", &[1]),
        ("rect[x$=\"8\"]", &[2]),
        (":root rect:first-child", &[0]),
    ];
    for (selector, styled) in cases {
        let document = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 12 4">
                <style>rect {{ fill: #ff0000 }} {selector} {{ fill: #0000ff }}</style>
                <g>
                    <rect class="plain" width="4" height="4"/>
                    <rect x="4" width="4" height="4" data-role="accent-strong"/>
                    <rect id="third" x="8" width="4" height="4"/>
                </g>
            </svg>"##
        );
        let raster = Svg::from_bytes(document.as_bytes())
            .expect("valid document")
            .render(12, 4)
            .expect("raster rendered");
        for index in 0..3 {
            let expected = match styled.contains(&index) {
                true => [0, 0, 255, 255],
                false => [255, 0, 0, 255],
            };
            assert_eq!(
                pixel(&raster, index * 4 + 2, 2),
                expected,
                "{selector}: rect {index}"
            );
        }
    }
}
