use std::sync::Arc;

use crate::support::Sheet;
use glam::{Mat4, Vec3};
use rikka_model_core::{Instance, ModelBuilder, Node, NodeId};
use rikka_physics_cloth::Cloth;
use rikka_physics_tailor::{
    Anchor, Body, BodyVertex, ClothAsset, Particle, Piece, PieceKey, PieceKind, Preset,
};

const HALF: f32 = 0.06;

fn boxed() -> (Arc<rikka_model_core::Model>, Arc<ClothAsset>) {
    let mut builder = ModelBuilder::new();
    builder.add_node(Node::new("box"));
    let model = Arc::new(builder.finish().expect("a model"));
    let corners: Vec<Vec3> = (0..8)
        .map(|at| {
            Vec3::new(
                if at & 1 == 0 { -HALF } else { HALF },
                if at & 2 == 0 { -HALF } else { HALF },
                if at & 4 == 0 { -HALF } else { HALF },
            )
        })
        .collect();
    let faces = [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ];
    let triangles = faces
        .iter()
        .flat_map(|[a, b, c, d]| [[*a, *b, *c], [*a, *c, *d]])
        .collect();
    let skinned = Anchor::Skinned {
        joints: [NodeId(0); 4],
        weights: [1.0, 0.0, 0.0, 0.0],
    };
    let tag = |rest: Vec3| Particle {
        rest,
        anchor: skinned,
        pinned: true,
        mass: 0.01,
        depth: 0.0,
        reach: 0.0,
        parent: None,
        radius: 0.005,
        clearance: 1.0,
        render_offset: 0.0,
        relief_height: 0.0,
        relief_depth: 0.0,
        spread: 1.0,
    };
    let piece = Piece {
        key: PieceKey {
            node: NodeId(0),
            vertex: 0,
        },
        name: "tag".into(),
        kind: PieceKind::Sheet,
        preset: Preset::Accessory,
        style: crate::support::style(),
        layer: 0,
        particles: vec![
            tag(Vec3::new(0.0, HALF, 0.0)),
            tag(Vec3::new(0.01, HALF, 0.0)),
            tag(Vec3::new(0.0, HALF, 0.01)),
        ],
        edges: Vec::new(),
        bends: Vec::new(),
        tethers: Vec::new(),
        triangles: vec![[0, 1, 2]],
        strand_nodes: Vec::new(),
        bindings: Vec::new(),
    };
    let asset = ClothAsset {
        rest_worlds: model.world_transforms(),
        pieces: vec![piece],
        body: Body {
            vertices: corners
                .into_iter()
                .map(|rest| BodyVertex {
                    rest,
                    joints: [NodeId(0); 4],
                    weights: [1.0, 0.0, 0.0, 0.0],
                })
                .collect(),
            triangles,
            colliders: Vec::new(),
        },
    };
    (model, Arc::new(asset))
}

#[test]
fn body_pushes_other_figures_cloth() {
    let sheet = Sheet::new(10, 10, 0.02, PieceKind::Sheet);
    let (model, asset) = boxed();
    let mut cloth = crate::support::cloth(Cloth::builder());
    let hanging = cloth
        .add_styled_figure(sheet.asset.clone(), [crate::support::style()])
        .expect("the sheet");
    let pushing = cloth.add_figure(asset).expect("the box");
    let still = sheet.instance();
    let mut mover = Instance::new(model);
    let mut deepest = f32::MIN;
    let mut farthest = 0.0f32;
    for frame in 0..150u32 {
        let travelled = (frame as f32 / 90.0).min(1.0);
        let centre = Vec3::new(0.0, -0.1, 0.25 - 0.5 * travelled);
        mover.set_transform(Mat4::from_translation(centre));
        let inputs = rikka_physics_cloth::Inputs::new()
            .figure(hanging, &still)
            .figure(pushing, &mover);
        cloth
            .advance(f64::from(frame) / 60.0, &inputs)
            .expect("a step");
        for position in crate::support::positions(&mut cloth, hanging) {
            let local = position - centre;
            deepest = deepest.max(HALF - local.abs().max_element());
            farthest = farthest.max(-position.z);
        }
    }
    println!(
        "the sheet was pushed {farthest:.3} m back, at worst {:.1} mm inside the box",
        deepest * 1000.0
    );
    assert!(farthest > 0.05, "the box went through the sheet");
    assert!(
        deepest < 0.004,
        "a particle sank {:.1} mm into the box",
        deepest * 1000.0
    );
}
