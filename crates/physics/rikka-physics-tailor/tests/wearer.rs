use rikka_humanoid::Anatomy;
use rikka_physics_tailor::{ClothAnalysis, ClothSettings, PieceKind, Preset, Wearer};

use crate::support::miku;

#[test]
fn skirt_gives_way_from_pinned_waist_to_hem_within_backstop() {
    let model = miku();
    let anatomy = Anatomy::recognize(model);
    let analysis =
        ClothAnalysis::new(model, &anatomy, &ClothSettings::new()).expect("cloth analysis");
    let wearer = Wearer::from_model(model, &anatomy, &analysis).expect("wearer");
    let sheets = analysis
        .pieces()
        .iter()
        .filter(|piece| piece.kind == PieceKind::Sheet)
        .count();
    assert_eq!(wearer.garments.len(), sheets, "only sheets are worn");

    let piece = analysis
        .pieces()
        .iter()
        .find(|piece| piece.preset == Preset::Skirt)
        .expect("miku wears a skirt");
    let skirt = wearer
        .garments
        .iter()
        .find(|garment| garment.piece == piece.key)
        .expect("the skirt is worn");
    let tip = piece
        .style
        .backstop_at(PieceKind::Sheet, 1.0)
        .expect("skirts keep a backstop");
    assert!(
        skirt.vertices.iter().any(|vertex| vertex.give == 0.0),
        "pinned vertices never give way"
    );
    assert!(
        skirt.vertices.iter().all(|vertex| vertex.give <= tip),
        "no vertex gives way past the backstop"
    );
    let hem = skirt
        .vertices
        .iter()
        .min_by(|one, other| one.rest.y.total_cmp(&other.rest.y))
        .expect("the skirt has vertices");
    assert!(
        hem.give > 0.5 * tip,
        "the hem gives way most: {} of {tip}",
        hem.give
    );
}
