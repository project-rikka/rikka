use rikka_psd::{
    ColorMode, Compression, Descriptor, Document, EngineValue, Info, Layer, Pattern, Plane,
    TypeTool, Value,
};

const ENGINE: &[u8] = b"\n\n<<\n\t/EngineDict\n\t<<\n\t\t/Editor\n\t\t<<\n\t\t\t/Text (\xfe\xff\x00H\x00i\x00\\(\x00\r)\n\t\t>>\n\t\t/StyleRun\n\t\t<<\n\t\t\t/RunArray [\n\t\t\t<<\n\t\t\t\t/StyleSheet\n\t\t\t\t<<\n\t\t\t\t\t/StyleSheetData\n\t\t\t\t\t<<\n\t\t\t\t\t\t/Font 0\n\t\t\t\t\t\t/FontSize 24.0\n\t\t\t\t\t\t/FillColor\n\t\t\t\t\t\t<<\n\t\t\t\t\t\t\t/Type 1\n\t\t\t\t\t\t\t/Values [ 1.0 .5 0.0 1.0 ]\n\t\t\t\t\t\t>>\n\t\t\t\t\t>>\n\t\t\t\t>>\n\t\t\t>>\n\t\t\t]\n\t\t\t/RunLengthArray [ 4 ]\n\t\t>>\n\t\t/ParagraphRun\n\t\t<<\n\t\t\t/RunArray [\n\t\t\t<<\n\t\t\t\t/ParagraphSheet\n\t\t\t\t<<\n\t\t\t\t\t/Properties\n\t\t\t\t\t<<\n\t\t\t\t\t\t/Justification 2\n\t\t\t\t\t>>\n\t\t\t\t>>\n\t\t\t>>\n\t\t\t]\n\t\t\t/RunLengthArray [ 4 ]\n\t\t>>\n\t>>\n\t/ResourceDict\n\t<<\n\t\t/FontSet [\n\t\t<<\n\t\t\t/Name (\xfe\xff\x00A\x00r\x00i\x00a\x00l)\n\t\t\t/Script 0\n\t\t\t/Synthetic false\n\t\t>>\n\t\t]\n\t>>\n>>";

fn text_layer() -> Layer {
    let mut layer = Layer::new("Hi(");
    layer.set_info(Info::TypeTool(TypeTool {
        version: 1,
        transform: [1.0, 0.0, 0.0, 1.0, 10.0, 20.0],
        text_version: 50,
        text: Descriptor::new("TxLr")
            .with("Txt ", "Hi(")
            .with("EngineData", Value::RawData(ENGINE.to_vec())),
        warp_version: 1,
        warp: Descriptor::new("warp"),
        bounds: [0.0, -20.0, 40.0, 5.0],
    }));
    layer
}

#[test]
fn engine_data_parses_and_serializes_stably() {
    let value = EngineValue::parse(ENGINE).unwrap();
    assert_eq!(
        value
            .path(&["EngineDict", "Editor", "Text"])
            .and_then(EngineValue::as_str),
        Some("Hi(\r")
    );
    let bytes = value.to_bytes();
    assert_eq!(EngineValue::parse(&bytes).unwrap(), value);
}

#[test]
fn text_layers_expose_and_replace_text() {
    let layer = text_layer();
    let text = layer.text().unwrap().unwrap();
    assert_eq!(text.text(), "Hi(");
    assert_eq!(text.fonts(), ["Arial"]);
    let runs = text.style_runs();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].style.size, Some(24.0));
    assert_eq!(runs[0].style.font.as_deref(), Some("Arial"));
    assert_eq!(
        text.paragraph_runs()[0].justification,
        rikka_psd::Justification::Center
    );

    let mut edited = text.clone();
    edited.set_text("Line one\nLine two");
    edited.set_font_size(12.0);
    let mut layer = layer.clone();
    layer.set_info(edited.to_info());
    let mut document = Document::new(64, 64, rikka_psd::Depth::Eight, ColorMode::Rgb);
    document.layers.push(layer.into());
    let read = Document::read(&document.to_bytes().unwrap()).unwrap();
    let text = read.layers[0].layer().text().unwrap().unwrap();
    assert_eq!(text.text(), "Line one\nLine two");
    let paragraphs = text.paragraph_runs();
    assert_eq!(
        paragraphs.iter().map(|run| run.length).collect::<Vec<_>>(),
        [9, 9]
    );
    assert_eq!(text.style_runs()[0].style.size, Some(12.0));
}

#[test]
fn pattern_files_round_trip() {
    let color: Vec<Plane> = (0..3)
        .map(|channel| {
            Plane::from_bytes(3, 2, rikka_psd::Depth::Eight, &[channel, 1, 2, 3, 4, 5]).unwrap()
        })
        .collect();
    let alpha =
        Plane::from_bytes(3, 2, rikka_psd::Depth::Eight, &[255, 0, 255, 0, 255, 0]).unwrap();
    let pattern = Pattern::new(
        "Dots",
        "3f2a9c1e-0000-4000-8000-000000000001",
        ColorMode::Rgb,
        &color,
        Some(&alpha),
    )
    .unwrap();
    let bytes = Pattern::write_pat(std::slice::from_ref(&pattern)).unwrap();
    let read = Pattern::read_pat(&bytes).unwrap();
    assert_eq!(read, vec![pattern.clone()]);
    let pixels = read[0].pixels().unwrap();
    assert_eq!(pixels.color, color);
    assert_eq!(pixels.alpha, Some(alpha));

    let mut document = Document::new(8, 8, rikka_psd::Depth::Eight, ColorMode::Rgb);
    document.set_info(Info::Patterns {
        key: rikka_psd::Key(*b"Patt"),
        patterns: vec![pattern],
    });
    document
        .set_composite(
            &[
                Plane::filled(8, 8, rikka_psd::Depth::Eight, 1.0),
                Plane::filled(8, 8, rikka_psd::Depth::Eight, 1.0),
                Plane::filled(8, 8, rikka_psd::Depth::Eight, 1.0),
            ],
            Compression::Rle,
        )
        .unwrap();
    let read = Document::read(&document.to_bytes().unwrap()).unwrap();
    assert_eq!(read.patterns().count(), 1);
    assert_eq!(read.patterns().next().unwrap().name, "Dots");
}
