use glam::{DVec2, Mat4, Quat, Vec3};
use rikka_mesh_boolean::{
    Boolean, Cdt, CdtOutput, IntersectBoolean, IntersectKnife, IntersectSource, Operand, Operation,
    SeparateMode, Solver,
};
use rikka_mesh_edit::{EditMesh, Primitive};
use rikka_polymesh::Polymesh;
use rikka_test_meshes::{
    EditOperator, Rng, Sweep, check_polymesh, corpus, is_polymesh_finite, outcome, sample_fixtures,
    sweep_chains, sweep_edit,
};

const CASES: usize = 24;
const SAMPLE_CASES: usize = 6;
const CHAINS: usize = 12;
const CHAIN_LENGTH: usize = 4;
const BOOLEAN_CASES: usize = 24;
const CDT_CASES: usize = 2000;
const OPERATIONS: [Operation; 3] = [
    Operation::Intersect,
    Operation::Union,
    Operation::Difference,
];
const SOLVERS: [Solver; 2] = [Solver::Exact, Solver::Fast];
const OUTPUTS: [CdtOutput; 8] = [
    CdtOutput::Full,
    CdtOutput::Inside,
    CdtOutput::InsideWithHoles,
    CdtOutput::InsideWithHolesNonzero,
    CdtOutput::Constraints,
    CdtOutput::ConstraintsValidBmesh,
    CdtOutput::ConstraintsValidBmeshWithHoles,
    CdtOutput::ConstraintsValidBmeshWithHolesNonzero,
];

fn threshold(rng: &mut Rng) -> f32 {
    match rng.flip() {
        true => 0.0,
        false => rng.range(0.0, 0.01),
    }
}

fn operators() -> Vec<(&'static str, EditOperator)> {
    vec![
        ("intersect_knife", |mesh, rng| {
            let knife = IntersectKnife::new()
                .source(rng.pick(&[
                    IntersectSource::SelfIntersect,
                    IntersectSource::SelectedUnselected,
                ]))
                .separate(rng.pick(&[SeparateMode::All, SeparateMode::Cut, SeparateMode::Merge]))
                .threshold(threshold(rng))
                .solver(rng.pick(&SOLVERS));
            outcome(knife.apply(mesh))
        }),
        ("intersect_boolean", |mesh, rng| {
            let boolean = IntersectBoolean::new(rng.pick(&OPERATIONS))
                .swap(rng.flip())
                .self_intersection(rng.flip())
                .threshold(threshold(rng))
                .solver(rng.pick(&SOLVERS));
            outcome(boolean.apply(mesh))
        }),
    ]
}

fn cutter(rng: &mut Rng) -> Polymesh {
    let mut mesh = EditMesh::new();
    let primitive = match rng.below(3) {
        0 => Primitive::Cube {
            size: rng.range(0.2, 2.0),
        },
        1 => Primitive::IcoSphere {
            subdivisions: rng.int(1, 2),
            radius: rng.range(0.2, 1.2),
        },
        _ => Primitive::Plane {
            size: rng.range(0.2, 2.0),
        },
    };
    if let Err(error) = mesh.add_primitive(&primitive, Mat4::IDENTITY, false) {
        panic!("cutter primitive: {error}");
    }
    mesh.to_polymesh()
}

fn transform(rng: &mut Rng) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::splat(rng.range(0.5, 1.5)),
        Quat::from_axis_angle(rng.direction(), rng.range(0.0, 3.0)),
        rng.direction() * rng.range(0.0, 0.8),
    )
}

fn check_result(mesh: &Polymesh, finite: bool, faces: usize) -> Result<(), String> {
    check_polymesh(mesh, finite)?;
    match faces == mesh.face_count() {
        true => Ok(()),
        false => Err("face origins do not match the result faces".to_owned()),
    }
}

#[test]
fn intersect_survives_corpus() {
    sweep_edit("intersect corpus", &corpus(), CASES, &operators());
}

#[test]
fn intersect_survives_samples() {
    sweep_edit(
        "intersect samples",
        &sample_fixtures(),
        SAMPLE_CASES,
        &operators(),
    );
}

#[test]
fn intersect_chains_survive() {
    sweep_chains(
        "intersect chains",
        &corpus(),
        CHAINS,
        CHAIN_LENGTH,
        &operators(),
    );
}

#[test]
fn boolean_survives_corpus() {
    let mut fixtures = corpus();
    fixtures.extend(sample_fixtures());
    let mut sweep = Sweep::new("boolean");
    for (index, fixture) in fixtures.iter().enumerate() {
        for case in 0..BOOLEAN_CASES {
            let mut rng = Rng::new((index * 1000 + case) as u64);
            let operation = rng.pick(&OPERATIONS);
            let solver = rng.pick(&SOLVERS);
            let label = format!("{operation:?} {solver:?} on {} case {case}", fixture.name);
            let mut error = None;
            sweep.run(&label, || {
                let other = match rng.below(3) {
                    0 => fixture.mesh.clone(),
                    1 => fixtures[rng.below(fixtures.len())].mesh.clone(),
                    _ => cutter(&mut rng),
                };
                let finite = is_polymesh_finite(&fixture.mesh) && is_polymesh_finite(&other);
                let boolean = Boolean::new(operation)
                    .operand(Operand::new(&fixture.mesh))
                    .operand(Operand::new(&other).transform(transform(&mut rng)))
                    .solver(solver)
                    .self_intersection(rng.flip())
                    .hole_tolerant(rng.flip())
                    .threshold(threshold(&mut rng));
                match boolean.compute() {
                    Ok(result) => check_result(&result.mesh, finite, result.face_origins.len()),
                    Err(message) => {
                        error = Some(message.to_string());
                        Ok(())
                    }
                }
            });
            if let Some(message) = error {
                sweep.note(format!("boolean: {message}"));
            }
        }
    }
    sweep.finish();
}

#[test]
fn cdt_survives_random_input() {
    let mut sweep = Sweep::new("cdt");
    for case in 0..CDT_CASES {
        let mut rng = Rng::new(case as u64);
        let output = rng.pick(&OUTPUTS);
        let label = format!("cdt {output:?} case {case}");
        let mut error = None;
        sweep.run(&label, || {
            let count = rng.int(0, 24) as usize;
            let mut verts: Vec<DVec2> = (0..count)
                .map(|_| {
                    DVec2::new(
                        f64::from(rng.range(-1.0, 1.0)),
                        f64::from(rng.range(-1.0, 1.0)),
                    )
                })
                .collect();
            if count > 2 && rng.flip() {
                let duplicate = verts[rng.below(count)];
                verts.push(duplicate);
                let a = verts[0];
                let b = verts[1];
                verts.push((a + b) * 0.5);
            }
            let total = verts.len() as u32;
            let edges: Vec<[u32; 2]> = (0..rng.below(count + 1))
                .map(|_| {
                    [
                        rng.below(total as usize) as u32,
                        rng.below(total as usize) as u32,
                    ]
                })
                .collect();
            let faces: Vec<Vec<u32>> = (0..rng.below(4))
                .map(|_| {
                    (0..rng.int(3, 6))
                        .map(|_| rng.below(total as usize) as u32)
                        .collect()
                })
                .filter(|_| total > 0)
                .collect();
            let epsilon = match rng.flip() {
                true => 0.0,
                false => f64::from(rng.range(0.0, 0.01)),
            };
            match Cdt::new(verts)
                .edges(edges)
                .faces(faces)
                .epsilon(epsilon)
                .triangulate(output)
            {
                Ok(result) => {
                    let total = result.verts.len() as u32;
                    let valid = result.edges.iter().flatten().all(|&vert| vert < total)
                        && result.faces.iter().flatten().all(|&vert| vert < total)
                        && result.verts.iter().all(|vert| vert.is_finite());
                    match valid {
                        true => Ok(()),
                        false => Err("triangulation names a missing vertex".to_owned()),
                    }
                }
                Err(message) => {
                    error = Some(message.to_string());
                    Ok(())
                }
            }
        });
        if let Some(message) = error {
            sweep.note(format!("cdt: {message}"));
        }
    }
    sweep.finish();
}
