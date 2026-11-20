use std::sync::Arc;

use crate::support::Sheet;
use glam::{Mat4, Quat, Vec3};
use rikka_model_core::{NodeId, Transform};
use rikka_physics_cloth::Cloth;
use rikka_physics_tailor::{
    BindTarget, ClothAsset, PieceKind, StrandBinding, StrandNode, StrandWeight, VertexBinding,
};

const OWN: f32 = 0.5;
const NEIGHBOUR: f32 = 0.125;

fn stranded(sheet: &Sheet) -> Arc<ClothAsset> {
    let mut asset = (*sheet.asset).clone();
    let (columns, rows) = (sheet.columns, sheet.rows);
    let at = |row: u32, column: u32| row * columns + column;
    let strand_nodes: Vec<StrandNode> = (0..rows)
        .flat_map(|row| {
            (0..columns).map(move |column| StrandNode {
                origin: at(row, column),
                side: if column + 1 < columns {
                    at(row, column + 1)
                } else {
                    at(row, column - 1)
                },
                along: [
                    at(row.saturating_sub(1), column),
                    at((row + 1).min(rows - 1), column),
                ],
            })
        })
        .collect();
    let piece = &mut asset.pieces[0];
    let rest = piece.rest_positions();
    piece.bindings = piece
        .bindings
        .iter()
        .map(|bind| {
            let (row, column) = (bind.vertex / columns, bind.vertex % columns);
            let mut near = vec![(bind.vertex, OWN)];
            for (down, right) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let (row, column) = (row as i32 + down, column as i32 + right);
                if (0..rows as i32).contains(&row) && (0..columns as i32).contains(&column) {
                    near.push((at(row as u32, column as u32), NEIGHBOUR));
                }
            }
            let total: f32 = near.iter().map(|(_, weight)| weight).sum();
            let mut strand = StrandBinding::default();
            for (entry, (strand_node, weight)) in strand.weights.iter_mut().zip(near) {
                *entry = StrandWeight {
                    strand_node,
                    weight: weight / total,
                };
            }
            strand.offset = rest[bind.vertex as usize] - strand.origin(&strand_nodes, &rest);
            strand.normal = Vec3::Z;
            strand.tangent = Vec3::X;
            VertexBinding {
                target: BindTarget::Strand(strand),
                ..*bind
            }
        })
        .collect();
    piece.strand_nodes = strand_nodes;
    Arc::new(asset)
}

#[test]
fn resting_strand_drape_matches_skinned_mesh() {
    let sheet = Sheet::new(5, 9, 0.03, PieceKind::Strands);
    let mut cloth = crate::support::cloth(Cloth::builder().gravity(Vec3::ZERO));
    let figure = cloth
        .add_styled_figure(stranded(&sheet), [crate::support::style()])
        .expect("a figure");
    let mut instance = sheet.instance();
    let placement =
        Mat4::from_rotation_translation(Quat::from_rotation_z(0.4), Vec3::new(0.5, 1.0, -0.2));
    instance.set_transform(placement);
    let posed = Transform {
        translation: Vec3::new(0.0, 0.1, 0.05),
        rotation: Quat::from_rotation_x(0.7) * Quat::from_rotation_y(-0.3),
        scale: Vec3::ONE,
    };
    instance.pose_mut().set_local(NodeId(0), posed);
    instance.refresh();
    for frame in 0..20 {
        crate::support::step(&mut cloth, figure, &instance, frame);
    }
    let skinned = placement * posed.to_matrix();
    let drapes = cloth.drapes(figure).expect("drapes");
    let draped = crate::support::draped(&drapes[0]);
    let normal = skinned.transform_vector3(Vec3::Z).normalize();
    let tangent = skinned.transform_vector3(Vec3::X).normalize();
    for (vertex, rest) in sheet.rest().iter().enumerate() {
        let expected = skinned.transform_point3(*rest);
        let got = &draped[vertex];
        assert!(
            got.position.distance(expected) < 1.0e-4,
            "vertex {vertex}: {} against {expected}",
            got.position
        );
        assert!(got.normal.distance(normal) < 1.0e-3);
        assert!(got.tangent.truncate().distance(tangent) < 1.0e-3);
    }
}

#[test]
fn gpu_strand_drape_matches_cpu_reference() {
    let sheet = Sheet::new(5, 9, 0.03, PieceKind::Strands);
    let asset = stranded(&sheet);
    let mut cloth = crate::support::cloth(Cloth::builder());
    let figure = cloth
        .add_styled_figure(Arc::clone(&asset), [crate::support::style()])
        .expect("a figure");
    let mut instance = sheet.instance();
    for frame in 0..40 {
        let swing = Quat::from_rotation_z(0.8 * (frame as f32 * 0.15).sin());
        instance.pose_mut().set_local(
            NodeId(0),
            Transform {
                translation: Vec3::new(0.0, 0.0, 0.002 * frame as f32),
                rotation: swing,
                scale: Vec3::ONE,
            },
        );
        instance.refresh();
        crate::support::step(&mut cloth, figure, &instance, frame);
    }
    let positions = crate::support::positions(&mut cloth, figure);
    let expected = asset.pieces[0].drape(&positions);
    let drapes = cloth.drapes(figure).expect("drapes");
    let draped = crate::support::draped(&drapes[0]);
    let rest = sheet.rest();
    let moved = positions
        .iter()
        .zip(&rest)
        .map(|(now, then)| now.distance(*then))
        .fold(0.0f32, f32::max);
    assert!(moved > 0.05, "the strands never swung ({moved} m)");
    for (vertex, (got, expected)) in draped.iter().zip(&expected).enumerate() {
        assert!(
            got.position.distance(expected.position) < 1.0e-4,
            "vertex {vertex}: {} against {}",
            got.position,
            expected.position
        );
        assert!(got.normal.distance(expected.normal) < 1.0e-3);
        assert!(got.tangent.truncate().distance(expected.tangent) < 1.0e-3);
    }
}
