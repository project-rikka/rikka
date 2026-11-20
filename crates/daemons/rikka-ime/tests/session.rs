use rikka_ime::{Caret, DEFAULT_LAYOUT, InputSource, Keymap, PanelAnchor};

#[test]
fn input_sources_compile_as_xkb_groups() {
    let keymap = Keymap::from_sources(
        &[
            InputSource::layout("us"),
            InputSource::layout("jp").variant("kana"),
            InputSource::layout("de"),
        ],
        "pc105",
        Some(String::new()),
    );
    assert_eq!(keymap.layout, "us,jp,de");
    assert_eq!(keymap.variant, ",kana,");
    assert_eq!(keymap.describe(), "us,jp(kana),de");
    assert!(keymap.options.is_none());

    let many: Vec<InputSource> = (0..6).map(|_| InputSource::layout("us")).collect();
    assert_eq!(
        Keymap::from_sources(&many, "pc105", None).layout,
        "us,us,us,us"
    );

    assert_eq!(
        Keymap::from_sources(&[], "pc105", None).layout,
        DEFAULT_LAYOUT
    );
}

#[test]
fn compositor_aim_parses() {
    let caret: PanelAnchor = serde_json::from_value(serde_json::json!({
        "at": "caret",
        "output": "DP-1",
        "caret": { "x": 1920, "y": 300, "width": 2, "height": 20 },
    }))
    .expect("session shape");
    assert_eq!(
        caret,
        PanelAnchor::Caret {
            output: "DP-1".to_owned(),
            caret: Caret {
                x: 1920,
                y: 300,
                width: 2,
                height: 20,
            },
        }
    );

    let pointer: PanelAnchor = serde_json::from_value(
        serde_json::json!({ "at": "pointer", "output": "eDP-1", "x": 40, "y": 60 }),
    )
    .expect("session shape");
    assert_eq!(pointer.output(), "eDP-1");
    assert_eq!(
        pointer,
        PanelAnchor::Pointer {
            output: "eDP-1".to_owned(),
            x: 40,
            y: 60,
        }
    );
}
