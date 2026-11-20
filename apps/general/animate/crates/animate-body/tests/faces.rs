use animate_body::{Expression, Faces, Morph, MorphWeights, Panel, group_sides};
use rikka_mmd::{Pmx, PmxMorph, PmxOffsets, PmxPanel};
use rikka_model_core::{Instance, ModelBuilder, Node};

use crate::support::miku;

#[test]
fn pmx_morphs_list_by_panel() {
    let model = miku();
    let faces = Faces::from_model(&model);
    let pmx = model.extensions.get::<Pmx>().expect("a PMX");
    assert_eq!(faces.morphs().len(), pmx.morphs.len());
    for (morph, source) in faces.morphs().iter().zip(&pmx.morphs) {
        assert_eq!(morph.name(), source.name);
        let panel = match source.panel {
            PmxPanel::Hidden => None,
            PmxPanel::Eyebrow => Some(Panel::Eyebrow),
            PmxPanel::Eye => Some(Panel::Eye),
            PmxPanel::Mouth => Some(Panel::Mouth),
            PmxPanel::Other => Some(Panel::Other),
        };
        assert_eq!(morph.panel(), panel, "{}", source.name);
    }
    for expression in Expression::ALL {
        assert!(faces.has(expression), "{} resolves", expression.name());
    }
    assert_eq!(faces.resolve("まばたき"), Some("まばたき"));
    assert_eq!(faces.resolve(" まばたき "), Some("まばたき"));
    assert_eq!(faces.resolve("no such morph"), None);
}

#[test]
fn expression_presets_write_clamped_weights() {
    let model = miku();
    let faces = Faces::from_model(&model);
    let pmx = model.extensions.get::<Pmx>().expect("a PMX");
    let morph = |name: &str| {
        pmx.morphs
            .iter()
            .position(|morph| morph.name == name)
            .expect("a morph of miku")
    };

    let mut expressing = MorphWeights::default();
    faces.express(&mut expressing, Expression::Happy, 0.8);
    assert!((expressing.weight("笑い") - 0.8).abs() < 1e-6);
    assert!((expressing.weight("にこり") - 0.8).abs() < 1e-6);
    assert!((faces.strength(&expressing, Expression::Happy) - 0.8).abs() < 1e-6);
    expressing.set("あ", 1.5);
    expressing.set("はぅ", 0.25);
    expressing.set("no such morph", 1.0);

    let mut instance = Instance::new(std::sync::Arc::new(model.clone()));
    faces.write(&expressing, &mut instance);
    let weights = instance.pose().weights(pmx.root);
    assert_eq!(weights.len(), pmx.morphs.len());
    assert!((weights[morph("笑い")] - 0.8).abs() < 1e-6);
    assert!((weights[morph("あ")] - 1.0).abs() < 1e-6);
    assert!((weights[morph("はぅ")] - 0.25).abs() < 1e-6);
    assert_eq!(weights[morph("まばたき")], 0.0);
}

fn named(names: &[&str]) -> Faces {
    let mut builder = ModelBuilder::new();
    builder.add_node(Node::new("root"));
    let mut model = builder.finish().expect("one node is a model");
    model.extensions.insert(Pmx {
        morphs: names
            .iter()
            .map(|name| PmxMorph {
                name: (*name).to_owned(),
                english_name: String::new(),
                panel: PmxPanel::Other,
                offsets: PmxOffsets::Group(Vec::new()),
            })
            .collect(),
        ..Pmx::default()
    });
    Faces::from_model(&model)
}

#[test]
fn sided_morphs_group_by_shared_name() {
    let faces = named(&[
        "真面目",
        "真面目左",
        "真面目右",
        "上右",
        "eye_R",
        "eye_L",
        "ウィンク右",
        "ウィンク",
        "blink",
        "blinkLeft",
        "blinkRight",
        "BAR",
        "左手",
    ]);
    let name =
        |morph: Option<&Morph>| morph.map_or("-".to_owned(), |morph| morph.name().to_owned());
    let gathered: Vec<String> = group_sides(faces.morphs())
        .iter()
        .map(|sided| {
            format!(
                "{}: {} {} {}",
                sided.name,
                name(sided.whole),
                name(sided.left),
                name(sided.right)
            )
        })
        .collect();
    assert_eq!(
        gathered,
        [
            "真面目: 真面目 真面目左 真面目右",
            "上右: 上右 - -",
            "eye: - eye_L eye_R",
            "ウィンク: ウィンク - ウィンク右",
            "blink: blink blinkLeft blinkRight",
            "BAR: BAR - -",
            "左手: 左手 - -",
        ]
    );
}
