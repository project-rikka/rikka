use std::sync::Arc;

use crate::support::Sheet;
use glam::{Vec3, Vec4};
use rikka_model_core::{Mesh, Model, ModelBuilder, Node};
use rikka_physics_cloth::Cloth;
use rikka_physics_tailor::{BindTarget, ClothAsset, Piece, PieceKind};

const COLUMNS: u32 = 9;
const ROWS: u32 = 9;
const SPACING: f32 = 0.03;
const APART: f32 = 0.03;
const RELIEF: f32 = 0.012;
const SETTLED: u32 = 120;

struct Layered {
    model: Arc<Model>,
    asset: Arc<ClothAsset>,
    drawn: usize,
}

fn lying(point: Vec3, height: f32) -> Vec3 {
    Vec3::new(point.x, height, -point.y)
}

fn layer(sheet: &Sheet, height: f32, lift: f32, first: u32) -> Piece {
    let mut piece = sheet.asset.pieces[0].clone();
    for particle in &mut piece.particles {
        particle.rest = lying(particle.rest, height);
    }
    for bind in &mut piece.bindings {
        bind.vertex += first;
        if let BindTarget::Triangles(influences) = &mut bind.target {
            influences[0].offset.z = lift;
        }
    }
    piece
}

fn layered() -> Layered {
    let sheet = Sheet::new(COLUMNS, ROWS, SPACING, PieceKind::Sheet);
    let drawn = sheet.rest().len();
    let mut inner = layer(&sheet, 0.0, RELIEF, 0);
    for particle in &mut inner.particles {
        particle.pinned = true;
        particle.relief_height = RELIEF;
    }
    let mut outer = layer(&sheet, APART, -RELIEF, drawn as u32);
    outer.layer = 1;
    for particle in &mut outer.particles {
        particle.relief_depth = RELIEF;
    }
    let rest = sheet.rest();
    let mut mesh = Mesh::new("layers");
    mesh.vertices.positions = rest
        .iter()
        .map(|point| lying(*point, RELIEF))
        .chain(rest.iter().map(|point| lying(*point, APART - RELIEF)))
        .collect();
    mesh.vertices.normals = vec![Vec3::Y; 2 * drawn];
    mesh.vertices.tangents = vec![Vec4::new(1.0, 0.0, 0.0, 1.0); 2 * drawn];
    mesh.indices = inner
        .triangles
        .iter()
        .flatten()
        .copied()
        .chain(
            outer
                .triangles
                .iter()
                .flatten()
                .map(|corner| corner + drawn as u32),
        )
        .collect();
    let mut builder = ModelBuilder::new();
    let mesh = builder.add_mesh(mesh);
    builder.add_node(Node::new("root").mesh(mesh));
    let model = Arc::new(builder.finish().expect("a layered model"));
    let asset = ClothAsset {
        rest_worlds: model.world_transforms(),
        pieces: vec![inner, outer],
        body: sheet.asset.body.clone(),
    };
    Layered {
        model,
        asset: Arc::new(asset),
        drawn,
    }
}

#[test]
fn outer_layer_rests_on_inner_relief() {
    let layered = layered();
    let mut cloth = crate::support::cloth(Cloth::builder());
    let figure = cloth
        .add_figure(Arc::clone(&layered.asset))
        .expect("a figure");
    let instance = rikka_model_core::Instance::new(Arc::clone(&layered.model));
    for frame in 0..SETTLED {
        crate::support::step(&mut cloth, figure, &instance, frame);
    }
    let particles = crate::support::positions(&mut cloth, figure);
    let outer = &particles[particles.len() / 2..];
    let sunk = outer
        .iter()
        .skip(COLUMNS as usize)
        .map(|particle| APART - particle.y)
        .fold(0.0f32, f32::max);
    let free_fall = APART - 2.0 * RELIEF;
    assert!(
        sunk > 0.5 * free_fall,
        "the outer sheet sank only {sunk} m of its {free_fall} m onto the inner one"
    );
    let drape = cloth
        .drapes(figure)
        .expect("drapes")
        .into_iter()
        .next()
        .expect("a drape");
    let drawn = crate::support::draped(&drape);
    let half_width = 0.5 * (COLUMNS - 1) as f32 * SPACING;
    let depth = (ROWS - 1) as f32 * SPACING;
    let over_inner = |at: Vec3| at.x.abs() < half_width - SPACING && at.z < depth - SPACING;
    let covering: Vec<f32> = drawn[layered.drawn..]
        .iter()
        .filter(|vertex| over_inner(vertex.position))
        .map(|vertex| vertex.position.y)
        .collect();
    assert!(covering.len() > layered.drawn / 2);
    let lowest = covering.iter().copied().fold(f32::INFINITY, f32::min);
    assert!(
        lowest > RELIEF,
        "the outer recess sank to {lowest} m, under the inner relief at {RELIEF} m"
    );
}
