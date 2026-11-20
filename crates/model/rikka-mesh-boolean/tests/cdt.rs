use glam::DVec2;
use rikka_mesh_boolean::{Cdt, CdtOutput, CdtResult, EdgeOrigin};

fn signed_area(result: &CdtResult, face: &[u32]) -> f64 {
    let mut area = 0.0;
    for i in 0..face.len() {
        let a = result.verts[face[i] as usize];
        let b = result.verts[face[(i + 1) % face.len()] as usize];
        area += a.perp_dot(b);
    }
    area * 0.5
}

fn total_area(result: &CdtResult) -> f64 {
    result
        .faces
        .iter()
        .map(|face| signed_area(result, face))
        .sum()
}

fn points(coords: &[(f64, f64)]) -> Vec<DVec2> {
    coords.iter().map(|&(x, y)| DVec2::new(x, y)).collect()
}

fn in_circle(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> f64 {
    let (ad, bd, cd) = (a - d, b - d, c - d);
    ad.length_squared() * bd.perp_dot(cd)
        + bd.length_squared() * cd.perp_dot(ad)
        + cd.length_squared() * ad.perp_dot(bd)
}

#[test]
fn full_output_covers_convex_hull() {
    let verts = points(&[
        (0.0, 0.0),
        (4.0, 0.0),
        (4.0, 3.0),
        (0.0, 3.0),
        (1.0, 1.0),
        (3.0, 2.0),
    ]);
    let result = Cdt::new(verts).triangulate(CdtOutput::Full).unwrap();
    assert_eq!(result.faces.len(), 6);
    assert!(
        result
            .faces
            .iter()
            .all(|face| signed_area(&result, face) > 0.0)
    );
    assert!((total_area(&result) - 12.0).abs() < 1e-12);
    assert_eq!(result.edges.len(), 11);
}

#[test]
fn triangulation_is_delaunay() {
    let mut verts = Vec::new();
    let mut seed = 12345u64;
    for _ in 0..400 {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let x = (seed >> 33) as f64 / (1u64 << 31) as f64;
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let y = (seed >> 33) as f64 / (1u64 << 31) as f64;
        verts.push(DVec2::new(x, y));
    }
    let result = Cdt::new(verts.clone())
        .triangulate(CdtOutput::Full)
        .unwrap();
    assert_eq!(result.faces.len(), 2 * 400 - 2 - hull_size(&result));
    for face in &result.faces {
        let [a, b, c] = [0, 1, 2].map(|i| result.verts[face[i] as usize]);
        for &point in &result.verts {
            assert!(in_circle(a, b, c, point) <= 1e-9);
        }
    }
}

fn hull_size(result: &CdtResult) -> usize {
    let mut count = std::collections::HashMap::new();
    for face in &result.faces {
        for i in 0..3 {
            let (a, b) = (face[i], face[(i + 1) % 3]);
            *count.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
    }
    count.values().filter(|&&n| n == 1).count()
}

#[test]
fn collinear_points_give_only_edges() {
    let verts = points(&[(0.0, 0.0), (3.0, 3.0), (1.0, 1.0), (2.0, 2.0), (1.0, 1.0)]);
    let result = Cdt::new(verts)
        .edges([[0, 1]])
        .triangulate(CdtOutput::Full)
        .unwrap();
    assert!(result.faces.is_empty());
    assert_eq!(result.verts.len(), 4);
    assert_eq!(result.vert_origins[2], vec![2, 4]);
    assert_eq!(result.edges.len(), 3);
    assert!(
        result
            .edge_origins
            .iter()
            .all(|origins| origins == &[EdgeOrigin::Edge(0)])
    );
}

#[test]
fn collinear_constraint_splits_at_vertices() {
    let verts = points(&[
        (0.0, 0.0),
        (4.0, 0.0),
        (1.0, 0.0),
        (3.0, 0.0),
        (2.0, 2.0),
        (2.0, -2.0),
    ]);
    let result = Cdt::new(verts)
        .edges([[0, 1]])
        .triangulate(CdtOutput::Full)
        .unwrap();
    let constrained: Vec<&[u32; 2]> = result
        .edges
        .iter()
        .zip(&result.edge_origins)
        .filter(|(_, origins)| !origins.is_empty())
        .map(|(edge, _)| edge)
        .collect();
    assert_eq!(constrained.len(), 3);
}

#[test]
fn duplicate_constraints_share_an_edge() {
    let verts = points(&[(0.0, 0.0), (2.0, 0.0), (1.0, 1.0), (1.0, -1.0)]);
    let result = Cdt::new(verts)
        .edges([[2, 3], [3, 2], [2, 3]])
        .triangulate(CdtOutput::Full)
        .unwrap();
    let position = result
        .edges
        .iter()
        .position(|edge| (edge[0] == 2 && edge[1] == 3) || (edge[0] == 3 && edge[1] == 2))
        .unwrap();
    assert_eq!(
        result.edge_origins[position],
        vec![
            EdgeOrigin::Edge(0),
            EdgeOrigin::Edge(1),
            EdgeOrigin::Edge(2)
        ]
    );
    assert_eq!(result.faces.len(), 2);
}

#[test]
fn crossing_constraints_meet_at_exact_point() {
    let verts = points(&[(0.0, 0.0), (3.0, 1.0), (0.0, 1.0), (3.0, 0.0), (1.0, 2.0)]);
    let result = Cdt::new(verts)
        .edges([[0, 1], [2, 3]])
        .triangulate(CdtOutput::Full)
        .unwrap();
    assert_eq!(result.verts.len(), 6);
    assert_eq!(result.verts[5], DVec2::new(1.5, 0.5));
    let constrained = result.edge_origins.iter().filter(|o| !o.is_empty()).count();
    assert_eq!(constrained, 4);
    assert!((total_area(&result) - 4.5).abs() < 1e-12);
}

#[test]
fn many_crossing_constraints_stay_consistent() {
    let mut verts = Vec::new();
    let mut edges = Vec::new();
    for i in 0..12 {
        let t = i as f64 * 0.37;
        verts.push(DVec2::new(t.cos() * 3.0, t.sin() * 3.0));
        verts.push(DVec2::new(-(t.cos()) * 3.1, -(t.sin()) * 2.9));
        edges.push([2 * i, 2 * i + 1]);
    }
    let result = Cdt::new(verts)
        .edges(edges)
        .triangulate(CdtOutput::Full)
        .unwrap();
    assert!(
        result
            .faces
            .iter()
            .all(|face| signed_area(&result, face) > 0.0)
    );
    let degree_sum: usize = result.faces.len() * 3;
    assert!(degree_sum > 0);
    for origins in &result.edge_origins {
        assert!(origins.len() <= 1);
    }
}

#[test]
fn inside_output_triangulates_concave_polygon() {
    let verts = points(&[
        (0.0, 0.0),
        (4.0, 0.0),
        (4.0, 1.0),
        (1.0, 1.0),
        (1.0, 4.0),
        (0.0, 4.0),
    ]);
    let result = Cdt::new(verts)
        .faces([vec![0, 1, 2, 3, 4, 5]])
        .triangulate(CdtOutput::Inside)
        .unwrap();
    assert_eq!(result.faces.len(), 4);
    assert!((total_area(&result) - 7.0).abs() < 1e-12);
    assert!(result.face_origins.iter().all(|origins| origins == &[0]));
}

#[test]
fn clockwise_face_is_labelled() {
    let verts = points(&[(0.0, 0.0), (0.0, 2.0), (2.0, 2.0), (2.0, 0.0)]);
    let result = Cdt::new(verts)
        .faces([vec![0, 1, 2, 3]])
        .triangulate(CdtOutput::Inside)
        .unwrap();
    assert_eq!(result.faces.len(), 2);
    assert!(result.face_origins.iter().all(|origins| origins == &[0]));
}

#[test]
fn even_odd_rule_cuts_holes() {
    let verts = points(&[
        (0.0, 0.0),
        (6.0, 0.0),
        (6.0, 6.0),
        (0.0, 6.0),
        (2.0, 2.0),
        (4.0, 2.0),
        (4.0, 4.0),
        (2.0, 4.0),
    ]);
    let faces = [vec![0, 1, 2, 3], vec![4, 5, 6, 7]];
    let holes = Cdt::new(verts.clone())
        .faces(faces.clone())
        .triangulate(CdtOutput::InsideWithHoles)
        .unwrap();
    assert!((total_area(&holes) - 32.0).abs() < 1e-12);
    let filled = Cdt::new(verts.clone())
        .faces(faces.clone())
        .triangulate(CdtOutput::InsideWithHolesNonzero)
        .unwrap();
    assert!((total_area(&filled) - 36.0).abs() < 1e-12);
    let inside = Cdt::new(verts)
        .faces(faces)
        .triangulate(CdtOutput::Inside)
        .unwrap();
    assert!((total_area(&inside) - 36.0).abs() < 1e-12);
}

#[test]
fn constraints_output_merges_faces() {
    let verts = points(&[
        (0.0, 0.0),
        (4.0, 0.0),
        (4.0, 4.0),
        (0.0, 4.0),
        (2.0, -1.0),
        (2.0, 5.0),
    ]);
    let result = Cdt::new(verts)
        .faces([vec![0, 1, 2, 3]])
        .edges([[4, 5]])
        .triangulate(CdtOutput::Constraints)
        .unwrap();
    assert_eq!(result.faces.len(), 2);
    assert!(result.faces.iter().all(|face| face.len() == 4));
    assert!((total_area(&result) - 16.0).abs() < 1e-12);
}

#[test]
fn valid_bmesh_output_keeps_hole_bridges() {
    let verts = points(&[
        (0.0, 0.0),
        (6.0, 0.0),
        (6.0, 6.0),
        (0.0, 6.0),
        (2.0, 2.0),
        (4.0, 2.0),
        (4.0, 4.0),
        (2.0, 4.0),
    ]);
    let result = Cdt::new(verts)
        .faces([vec![0, 1, 2, 3]])
        .edges([[4, 5], [5, 6], [6, 7], [7, 4]])
        .triangulate(CdtOutput::ConstraintsValidBmesh)
        .unwrap();
    for face in &result.faces {
        let mut sorted = face.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), face.len());
    }
    assert!((total_area(&result) - 36.0).abs() < 1e-12);
    assert!(result.faces.len() >= 3);
}

#[test]
fn epsilon_merges_near_vertices() {
    let verts = points(&[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0e-9, 0.0)]);
    let result = Cdt::new(verts)
        .epsilon(1.0e-6)
        .triangulate(CdtOutput::Full)
        .unwrap();
    assert_eq!(result.verts.len(), 3);
    assert_eq!(result.vert_origins[0], vec![0, 3]);
}

#[test]
fn rejects_out_of_range_indices() {
    let verts = points(&[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)]);
    assert!(
        Cdt::new(verts)
            .edges([[0, 7]])
            .triangulate(CdtOutput::Full)
            .is_err()
    );
}
