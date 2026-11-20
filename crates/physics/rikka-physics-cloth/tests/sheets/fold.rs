use crate::support::Sheet;
use glam::{Mat4, Vec3};
use rikka_physics_cloth::Cloth;
use rikka_physics_tailor::{PieceKind, Style};

fn crossing(p: Vec3, q: Vec3, [a, b, c]: [Vec3; 3]) -> bool {
    let n = (b - a).cross(c - a);
    let (dp, dq) = ((p - a).dot(n), (q - a).dot(n));
    if dp * dq >= 0.0 {
        return false;
    }
    let hit = p + (q - p) * (dp / (dp - dq));
    [(a, b), (b, c), (c, a)]
        .iter()
        .all(|(from, to)| (*to - *from).cross(hit - *from).dot(n) > 0.0)
}

#[test]
fn piled_sheet_does_not_self_intersect() {
    let sheet = Sheet::new(14, 20, 0.02, PieceKind::Sheet);
    let mut cloth = crate::support::cloth(Cloth::builder().floor(0.0));
    let style = Style {
        self_collision: true,
        thickness: 0.004,
        damping: rikka_physics_tailor::Curve::flat(0.05),
        ..crate::support::style()
    };
    let figure = cloth
        .add_styled_figure(sheet.asset.clone(), [style])
        .expect("a figure");
    let mut instance = sheet.instance();
    for frame in 0..360 {
        let lowered = (frame as f32 / 240.0).min(1.0);
        let height = 0.42 - 0.38 * lowered;
        let sideways = 0.08 * (frame as f32 / 30.0).sin() * lowered;
        instance.set_transform(Mat4::from_translation(Vec3::new(sideways, height, 0.0)));
        crate::support::step(&mut cloth, figure, &instance, frame);
    }
    let positions = crate::support::positions(&mut cloth, figure);
    assert!(positions.iter().all(|p| p.is_finite()));
    let piece = &sheet.asset.pieces[0];
    let lowest = positions.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    assert!(
        lowest > -0.001,
        "the sheet sank {lowest} m through the floor"
    );
    let mut crossings = 0;
    for edge in &piece.edges {
        let [i, j] = edge.ends;
        for triangle in &piece.triangles {
            if triangle.contains(&i) || triangle.contains(&j) {
                continue;
            }
            let corners = triangle.map(|corner| positions[corner as usize]);
            if crossing(positions[i as usize], positions[j as usize], corners) {
                crossings += 1;
            }
        }
    }
    let piled = positions.iter().filter(|p| p.y < 0.03).count();
    println!(
        "{piled} of {} particles lie on the floor, {crossings} edges cross the sheet",
        positions.len()
    );
    assert!(piled > positions.len() / 3, "the sheet did not pile up");
    assert_eq!(crossings, 0);
}
