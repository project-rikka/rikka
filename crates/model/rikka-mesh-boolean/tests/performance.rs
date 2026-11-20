use std::time::Instant;

use glam::Mat4;
use rikka_mesh_boolean::{Boolean, Operand, Operation, Solver};
use rikka_mesh_edit::Primitive;

use crate::support::{moved, primitive, stats};

#[test]
fn large_operands_finish() {
    let sphere = primitive(
        &Primitive::UvSphere {
            segments: 256,
            rings: 200,
            radius: 1.0,
        },
        Mat4::IDENTITY,
    );
    let triangles: usize = (0..sphere.face_count())
        .map(|face| sphere.face_size(face) - 2)
        .sum();
    for solver in [Solver::Exact, Solver::Fast] {
        let start = Instant::now();
        let result = Boolean::new(Operation::Difference)
            .operand(&sphere)
            .operand(Operand::new(&sphere).transform(moved([0.53, 0.31, 0.17])))
            .solver(solver)
            .compute()
            .unwrap();
        let elapsed = start.elapsed();
        let summary = stats(&result.mesh);
        eprintln!(
            "{solver:?} difference of two {triangles}-triangle spheres: {:.3} s, {} faces, volume {:.6}, manifold {}",
            elapsed.as_secs_f64(),
            summary.faces,
            summary.volume,
            summary.manifold
        );
        assert!(summary.manifold, "{solver:?} result is not closed");
        assert!(
            summary.volume > 0.5 && summary.volume < 4.2,
            "{solver:?} volume {}",
            summary.volume
        );
    }
}
