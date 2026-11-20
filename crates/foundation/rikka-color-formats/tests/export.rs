use rikka_color_formats::{Document, Grouping, Rgb, export};
use rikka_settings::session::color_picker::Format;

fn formats() -> Vec<Format> {
    vec![
        Format::new("HEX", "%Rex%Grx%Blx"),
        Format::new("RGB", "rgb(%Re, %Gr, %Bl)"),
        Format::new("Decimal", "%Dv"),
    ]
}

const COLORS: [Rgb; 2] = [Rgb::new(255, 0, 0), Rgb::new(0, 255, 0)];

#[test]
fn text_export_groups_by_colour_or_format() {
    assert_eq!(
        export(&COLORS, &formats(), Grouping::ByColor, Document::Text),
        "color1;HEX#ff0000;RGB(255, 0, 0);DECIMAL255\n\
         color2;HEX#00ff00;RGB(0, 255, 0);DECIMAL65280\n"
    );
    assert_eq!(
        export(&COLORS, &formats(), Grouping::ByFormat, Document::Text),
        "HEX;color1#ff0000;color2#00ff00\n\
         RGB;color1(255, 0, 0);color2(0, 255, 0)\n\
         DECIMAL;color1255;color265280\n"
    );
}

#[test]
fn json_export_keeps_input_order() {
    let colors: Vec<Rgb> = (0..11).map(|at| Rgb::new(at, 0, 0)).collect();
    let written = export(&colors, &formats()[..1], Grouping::ByColor, Document::Json);
    let read: serde_json::Value = serde_json::from_str(&written).expect("JSON");
    assert_eq!(read["color11"]["HEX"], "#0a0000");
    let keys: Vec<usize> = ["\"color1\"", "\"color2\"", "\"color10\"", "\"color11\""]
        .iter()
        .map(|key| written.find(key).expect("every colour"))
        .collect();
    assert!(keys.is_sorted(), "{written}");

    let by_format = export(&COLORS, &formats(), Grouping::ByFormat, Document::Json);
    let read: serde_json::Value = serde_json::from_str(&by_format).expect("JSON");
    assert_eq!(read["RGB"]["color2"], "(0, 255, 0)");
    assert!(by_format.find("\"HEX\"") < by_format.find("\"RGB\""));
}
