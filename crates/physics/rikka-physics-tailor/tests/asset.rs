use glam::Vec3;
use rikka_humanoid::Anatomy;
use rikka_model_core::Model;
use rikka_physics_tailor::{
    Anchor, BindTarget, BodyShape, Budget, ClothAsset, ClothSettings, Frame, PieceKind, Quality,
    particle_normals,
};

use crate::support::{Rest, avatar, avatar_asset, build_asset, miku, miku_asset};

const POSITION_TOLERANCE: f32 = 1e-4;
const NORMAL_TOLERANCE: f32 = 1.0;

fn samples() -> [(&'static str, &'static Model, &'static ClothAsset); 2] {
    [
        ("miku", miku(), miku_asset()),
        ("avatar", avatar(), avatar_asset()),
    ]
}

#[test]
fn cloth_asset_has_hair_garments_and_body() {
    for (name, _, asset) in samples() {
        assert!(
            asset
                .pieces
                .iter()
                .any(|piece| piece.kind == PieceKind::Strands && !piece.particles.is_empty()),
            "{name} has no hair"
        );
        assert!(
            asset
                .pieces
                .iter()
                .any(|piece| piece.kind == PieceKind::Sheet && !piece.particles.is_empty()),
            "{name} has no garment"
        );
        assert!(!asset.body.triangles.is_empty(), "{name} has no body");
        assert!(
            !asset.body.colliders.is_empty(),
            "{name} has no body colliders"
        );
        assert_eq!(asset.rest_worlds.len(), samples_nodes(name));
    }
}

fn samples_nodes(name: &str) -> usize {
    match name {
        "miku" => miku().nodes.len(),
        _ => avatar().nodes.len(),
    }
}

#[test]
fn budget_caps_particle_count() {
    for (name, model, asset) in samples() {
        assert!(
            asset.particle_count() <= Budget::default().particles as usize,
            "{name}"
        );
        let tight = Budget::from(Quality::Low).particles(900);
        let small = ClothAsset::build(
            model,
            &Anatomy::recognize(model),
            &ClothSettings::new(),
            tight,
        )
        .expect("cloth asset");
        assert!(
            small.particle_count() <= 900,
            "{name}: {}",
            small.particle_count()
        );
        assert!(!small.pieces.is_empty(), "{name}");
    }
}

#[test]
fn free_particles_are_tethered_to_pins() {
    for (name, _, asset) in samples() {
        for piece in &asset.pieces {
            let mut tethered = vec![false; piece.particles.len()];
            for tether in &piece.tethers {
                let particle = &piece.particles[tether.particle as usize];
                let anchor = &piece.particles[tether.anchor as usize];
                assert!(anchor.pinned && !particle.pinned, "{name} {}", piece.name);
                assert!(
                    tether.length + 1e-5 >= particle.rest.distance(anchor.rest),
                    "{name} {}: a tether is shorter than the straight line",
                    piece.name
                );
                tethered[tether.particle as usize] = true;
            }
            for (particle, is_tethered) in piece.particles.iter().zip(&tethered) {
                assert_eq!(particle.pinned, !is_tethered, "{name} {}", piece.name);
            }
            assert!(
                piece.particles.iter().any(|particle| particle.pinned),
                "{name} {}",
                piece.name
            );
        }
    }
}

#[test]
fn cloth_asset_values_finite_and_in_range() {
    let finite = |value: Vec3| value.is_finite();
    for (name, _, asset) in samples() {
        for piece in &asset.pieces {
            for particle in &piece.particles {
                assert!(finite(particle.rest), "{name} {}", piece.name);
                assert!(particle.mass > 0.0 && particle.mass.is_finite());
                assert!((0.0..=1.0).contains(&particle.depth));
                assert!(
                    particle.reach.is_finite()
                        && particle.radius > 0.0
                        && particle.clearance.is_finite()
                );
                assert!(particle.spread.is_finite());
                assert!(particle.relief_height >= 0.0 && particle.relief_height.is_finite());
                assert!(particle.relief_depth >= 0.0 && particle.relief_depth.is_finite());
                match particle.anchor {
                    Anchor::Skinned { weights, .. } => {
                        assert!(
                            (weights.iter().sum::<f32>() - 1.0).abs() < 1e-4,
                            "{name} {}",
                            piece.name
                        );
                    }
                    Anchor::Surface { uv, offset, .. } => assert!(uv.is_finite() && finite(offset)),
                }
            }
            assert!(
                piece
                    .edges
                    .iter()
                    .all(|edge| edge.rest.is_finite() && edge.rest > 0.0)
            );
            assert!(piece.bends.iter().all(|bend| bend.rest_angle.is_finite()));
            assert!(piece.tethers.iter().all(|tether| tether.length.is_finite()));
            for bind in &piece.bindings {
                assert!((0.0..=1.0).contains(&bind.blend));
                if let Some(under) = bind.underlay {
                    let inner = &asset.pieces[under.piece as usize];
                    assert!(inner.kind == PieceKind::Sheet && inner.layer < piece.layer);
                    assert!((under.triangle as usize) < inner.triangles.len());
                }
                match &bind.target {
                    BindTarget::Triangles(influences) => {
                        for influence in influences {
                            assert!(influence.uv.is_finite() && finite(influence.offset));
                            assert!(
                                finite(influence.normal)
                                    && finite(influence.tangent)
                                    && influence.weight.is_finite()
                            );
                        }
                    }
                    BindTarget::Strand(strand) => {
                        assert!(
                            finite(strand.offset)
                                && finite(strand.normal)
                                && finite(strand.tangent)
                        );
                        for entry in &strand.weights {
                            assert!(entry.weight.is_finite() && entry.weight >= 0.0);
                            assert!((entry.strand_node as usize) < piece.strand_nodes.len());
                        }
                    }
                }
            }
            for strand_node in &piece.strand_nodes {
                let within = |particle: u32| (particle as usize) < piece.particles.len();
                assert!(
                    within(strand_node.origin)
                        && within(strand_node.side)
                        && strand_node.along.iter().all(|end| within(*end))
                );
            }
        }
        for vertex in &asset.body.vertices {
            assert!(finite(vertex.rest));
        }
        for collider in &asset.body.colliders {
            match &collider.shape {
                BodyShape::Tapered { ends, radii } => {
                    assert!(
                        ends.iter().all(|end| end.is_finite())
                            && radii.iter().all(|radius| *radius > 0.0)
                    );
                }
                BodyShape::Hull { planes } => {
                    assert!(
                        planes
                            .iter()
                            .all(|(normal, offset)| normal.is_finite() && offset.is_finite())
                    );
                }
            }
        }
    }
}

#[test]
fn render_vertices_rebuild_from_rest_proxy() {
    for (name, model, asset) in samples() {
        let rest = Rest::new(model);
        for piece in &asset.pieces {
            assert!(!piece.bindings.is_empty(), "{name} {}", piece.name);
            let positions = piece.rest_positions();
            let normals = particle_normals(&positions, &piece.triangles);
            let draped = piece.drape(&positions);
            assert_eq!(draped.len(), piece.bindings.len());
            for (bind, draped) in piece.bindings.iter().zip(&draped) {
                let expected = rest.vertex(bind.node, bind.vertex);
                let (total, clamped): (f32, f32) = match &bind.target {
                    BindTarget::Triangles(influences) => (
                        influences.iter().map(|influence| influence.weight).sum(),
                        influences
                            .iter()
                            .filter(|influence| influence.weight > 0.0)
                            .map(|influence| {
                                let corners = piece.triangles[influence.triangle as usize];
                                let frame = Frame::from_triangle(
                                    corners.map(|corner| positions[corner as usize]),
                                    corners.map(|corner| normals[corner as usize]),
                                    influence.uv,
                                );
                                let depth = frame.local_point(expected.position).z;
                                (depth - influence.offset.z).abs() * influence.weight
                            })
                            .sum(),
                    ),
                    BindTarget::Strand(strand) => {
                        (strand.weights.iter().map(|entry| entry.weight).sum(), 0.0)
                    }
                };
                assert!(
                    (total - 1.0).abs() < 1e-5,
                    "{name} {}: weights sum to {total}",
                    piece.name
                );
                let gap = draped.position.distance(expected.position);
                assert!(
                    gap <= POSITION_TOLERANCE + clamped,
                    "{name} {}: vertex {} lands {gap} m away, its depth clamped by {clamped} m",
                    piece.name,
                    bind.vertex
                );
                if expected.normal != Vec3::ZERO {
                    let turn = draped.normal.angle_between(expected.normal).to_degrees();
                    assert!(
                        turn <= NORMAL_TOLERANCE,
                        "{name} {}: normal turned {turn}°",
                        piece.name
                    );
                }
            }
        }
    }
}

#[test]
fn build_is_deterministic() {
    assert_eq!(&build_asset(miku()), miku_asset());
    assert_eq!(&build_asset(avatar()), avatar_asset());
}
