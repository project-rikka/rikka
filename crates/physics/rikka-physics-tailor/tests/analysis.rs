use rikka_humanoid::Anatomy;
use rikka_mmd::Pmx;
use rikka_model_core::Model;
use rikka_physics_tailor::{
    AnalyzedPiece, Budget, ClothAnalysis, ClothAsset, ClothSettings, PieceKind, Preset, Role,
    VertexKey,
};

use crate::support::{avatar, miku};

fn has(analysis: &ClothAnalysis, kind: PieceKind, presets: &[Preset]) -> bool {
    analysis
        .pieces()
        .iter()
        .any(|piece| piece.kind == kind && presets.contains(&piece.preset))
}

const HAIR: [Preset; 4] = [
    Preset::LongHair,
    Preset::ShortHair,
    Preset::Bangs,
    Preset::Tail,
];

#[test]
fn analysis_finds_skirts_and_hair() {
    for (name, model) in [("miku", miku()), ("avatar", avatar())] {
        let analysis = ClothAnalysis::new(model, &Anatomy::recognize(model), &ClothSettings::new())
            .expect("cloth analysis");
        assert!(
            has(&analysis, PieceKind::Sheet, &[Preset::Skirt]),
            "{name}: {:#?}",
            names(&analysis)
        );
        assert!(
            has(&analysis, PieceKind::Strands, &HAIR),
            "{name}: {:#?}",
            names(&analysis)
        );
        assert!(
            analysis.vertices().any(|(_, role)| role == Role::Body),
            "{name}"
        );
        for piece in analysis.pieces() {
            assert!(piece.contains(piece.key.into()), "{name}: {}", piece.name);
            assert!(
                piece
                    .members()
                    .iter()
                    .any(|vertex| analysis.role(*vertex) == Some(Role::Free)),
                "{name}: {}",
                piece.name
            );
        }
    }
}

#[test]
fn analysis_ignores_bone_and_material_names() {
    let model = miku();
    let mut blind: Model = model.clone();
    for (index, node) in blind.nodes.iter_mut().enumerate() {
        node.name = format!("node {index}");
    }
    for (index, material) in blind.materials.iter_mut().enumerate() {
        material.name = format!("material {index}");
    }
    for mesh in &mut blind.meshes {
        mesh.name.clear();
        for submesh in &mut mesh.submeshes {
            submesh.name.clear();
        }
    }
    let pmx = blind.extensions.get_mut::<Pmx>().expect("a PMX model");
    for (index, bone) in pmx.bones.iter_mut().enumerate() {
        bone.name = format!("bone {index}");
        bone.english_name.clear();
    }
    for material in &mut pmx.materials {
        material.name.clear();
        material.english_name.clear();
    }
    let named = ClothAnalysis::new(model, &Anatomy::recognize(model), &ClothSettings::new())
        .expect("cloth analysis");
    let unnamed = ClothAnalysis::new(&blind, &Anatomy::recognize(&blind), &ClothSettings::new())
        .expect("cloth analysis");
    assert!(named.vertices().eq(unnamed.vertices()));
    let cut = |analysis: &ClothAnalysis| -> Vec<(PieceKind, Preset, Vec<VertexKey>)> {
        analysis
            .pieces()
            .iter()
            .map(|piece| (piece.kind, piece.preset, piece.members().to_vec()))
            .collect()
    };
    assert_eq!(cut(&named), cut(&unnamed), "{:#?}", names(&unnamed));
}

#[test]
fn piece_settings_follow_and_disable_piece() {
    let model = miku();
    let analysis = ClothAnalysis::new(model, &Anatomy::recognize(model), &ClothSettings::new())
        .expect("cloth analysis");
    let skirt = analysis
        .pieces()
        .iter()
        .find(|piece| piece.kind == PieceKind::Sheet && piece.preset == Preset::Skirt)
        .expect("a skirt");
    let mut cloth_settings = ClothSettings::new();
    let settings = cloth_settings.settings_mut(skirt);
    settings.enabled = false;
    settings.preset = Some(Preset::Cape);
    settings.key.vertex = u32::MAX;
    let folder = tempfile::tempdir().expect("a folder");
    let path = folder.path().join("miku.pmx.rcloth");
    cloth_settings.save(&path).expect("writable");
    let read = ClothSettings::load(&path).expect("readable");
    let again =
        ClothAnalysis::new(model, &Anatomy::recognize(model), &read).expect("cloth analysis");
    let moved = again
        .pieces()
        .iter()
        .find(|piece| piece.key == skirt.key)
        .expect("the skirt is still found");
    assert!(!moved.enabled);
    assert_eq!(moved.preset, Preset::Cape);
    assert_eq!(
        read.settings(moved).map(|settings| settings.enabled),
        Some(false)
    );
    let asset = ClothAsset::build(model, &Anatomy::recognize(model), &read, Budget::default())
        .expect("cloth asset");
    assert!(asset.pieces.iter().all(|piece| piece.key != skirt.key));
}

#[test]
fn painted_out_piece_stays_skinned() {
    let model = miku();
    let analysis = ClothAnalysis::new(model, &Anatomy::recognize(model), &ClothSettings::new())
        .expect("cloth analysis");
    let piece = analysis
        .pieces()
        .iter()
        .find(|piece| piece.kind == PieceKind::Strands)
        .expect("hair");
    let mut cloth_settings = ClothSettings::new();
    for vertex in piece.members() {
        cloth_settings.paint_role(*vertex, Some(Role::Skinned));
    }
    let painted = ClothAnalysis::new(model, &Anatomy::recognize(model), &cloth_settings)
        .expect("cloth analysis");
    assert!(painted.pieces().iter().all(|other| other.key != piece.key));
    assert!(
        piece
            .members()
            .iter()
            .all(|vertex| painted.role(*vertex) == Some(Role::Skinned))
    );
}

#[test]
fn hard_materials_never_become_cloth() {
    let mut model = miku().clone();
    let analysis = ClothAnalysis::new(&model, &Anatomy::recognize(&model), &ClothSettings::new())
        .expect("cloth analysis");
    let necktie = necktie(&analysis);
    let material = model
        .materials
        .iter_mut()
        .find(|material| material.name == necktie.name)
        .expect("necktie material");
    material.name = "metal".to_string();
    let hard = ClothAnalysis::new(&model, &Anatomy::recognize(&model), &ClothSettings::new())
        .expect("cloth analysis");
    assert_skinned(&hard, necktie.members());
}

fn necktie(analysis: &ClothAnalysis) -> &AnalyzedPiece {
    analysis
        .pieces()
        .iter()
        .find(|piece| piece.kind == PieceKind::Sheet && piece.preset == Preset::Ribbon)
        .expect("necktie piece")
}

fn assert_skinned(analysis: &ClothAnalysis, members: &[VertexKey]) {
    assert!(
        analysis
            .pieces()
            .iter()
            .all(|piece| members.iter().all(|vertex| !piece.contains(*vertex))),
        "{:#?}",
        names(analysis)
    );
    assert!(
        members
            .iter()
            .all(|vertex| analysis.role(*vertex) == Some(Role::Skinned))
    );
}

fn names(analysis: &ClothAnalysis) -> Vec<String> {
    analysis
        .pieces()
        .iter()
        .map(|piece| {
            format!(
                "{} {:?} {:?} {}",
                piece.name,
                piece.kind,
                piece.preset,
                piece.members().len()
            )
        })
        .collect()
}
