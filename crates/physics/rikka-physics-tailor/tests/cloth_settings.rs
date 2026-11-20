use rikka_humanoid::Anatomy;
use rikka_model_core::NodeId;
use rikka_physics_tailor::{
    AnalyzedPiece, ClothAnalysis, ClothSettings, ClothTuning, Preset, Role, VertexKey,
};

use crate::support::miku;

fn first_piece() -> AnalyzedPiece {
    ClothAnalysis::new(miku(), &Anatomy::recognize(miku()), &ClothSettings::new())
        .expect("cloth analysis")
        .pieces()
        .first()
        .expect("miku has cloth")
        .clone()
}

#[test]
fn settings_round_trip_through_sidecar() {
    let folder = tempfile::tempdir().expect("a folder");
    let model = folder.path().join("miku.pmx");
    let mut cloth_settings = ClothSettings::new();
    let vertex = VertexKey::new(NodeId(2), 17);
    cloth_settings.paint_role(vertex, Some(Role::Pinned));
    cloth_settings.paint_spread(vertex, Some(1.5));
    cloth_settings.paint_role(VertexKey::new(NodeId(0), 3), Some(Role::Body));
    let settings = cloth_settings.settings_mut(&first_piece());
    settings.enabled = false;
    settings.preset = Some(Preset::Cape);
    settings.style = Some(Preset::Ribbon.style());
    cloth_settings.set_tuning(ClothTuning {
        sway: 0.8,
        ..ClothTuning::NEUTRAL
    });

    cloth_settings.save_for_model(&model).expect("saved");
    assert!(folder.path().join("miku.pmx.rcloth").exists());
    let read = ClothSettings::load_for_model(&model).expect("readable");
    assert_eq!(read, cloth_settings);
    assert_eq!(read.paint_at(vertex).spread, Some(1.5));
}

#[test]
fn missing_sidecar_reads_as_new_and_malformed_is_rejected() {
    let folder = tempfile::tempdir().expect("a folder");
    let read = ClothSettings::load_for_model(&folder.path().join("absent.pmx"));
    assert_eq!(read.expect("no sidecar"), ClothSettings::new());
    let path = folder.path().join("broken.rcloth");
    for text in [
        "[1, 2]",
        r#"{"format":"rikka-cut","version":1,"content":{}}"#,
    ] {
        std::fs::write(&path, text).expect("the file writes");
        assert!(ClothSettings::load(&path).is_err(), "{text}");
    }
}

#[test]
fn only_tuning_and_style_preserve_topology() {
    let piece = first_piece();
    let mut plain = ClothSettings::new();
    plain.settings_mut(&piece);
    let mut restyled = plain.clone();
    let settings = restyled.settings_mut(&piece);
    settings.tuning.weight = 0.9;
    settings.style = Some(Preset::Cape.style());
    restyled.set_tuning(ClothTuning {
        response: 1.0,
        ..ClothTuning::NEUTRAL
    });
    assert!(restyled.same_topology(&plain));
    let mut moved = plain.clone();
    moved.settings_mut(&piece).enabled = false;
    assert!(!moved.same_topology(&plain));
    let mut painted = plain.clone();
    painted.paint_role(VertexKey::new(NodeId(1), 2), Some(Role::Free));
    assert!(!painted.same_topology(&plain));
}
