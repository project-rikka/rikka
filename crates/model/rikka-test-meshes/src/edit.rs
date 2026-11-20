use rikka_mesh_edit::EditMesh;

use crate::check::{check, is_finite};
use crate::corpus::Fixture;
use crate::rng::Rng;
use crate::samples::sample_patches;
use crate::selection::{Amount, Selection};
use crate::sweep::Sweep;

pub type EditOperator = fn(&mut EditMesh, &mut Rng) -> Result<(), String>;

const SAMPLE_PATCH_FACES: usize = 300;
const SAMPLE_PARTS: usize = 2;

#[must_use]
pub fn sample_fixtures() -> Vec<Fixture> {
    sample_patches(SAMPLE_PATCH_FACES, SAMPLE_PARTS)
}

fn seed(parts: &[&str], index: usize) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for byte in part.bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
        hash = (hash ^ 0xff).wrapping_mul(0x0100_0000_01b3);
    }
    hash ^ index as u64
}

fn selection(index: usize, rng: &mut Rng) -> Selection {
    let every = Selection::every();
    match index {
        0 => Selection {
            mode: every[0].mode,
            amount: Amount::All,
            hidden: false,
        },
        1 => Selection {
            mode: every[every.len() - 1].mode,
            amount: Amount::Partial,
            hidden: true,
        },
        _ => Selection::random(rng),
    }
}

pub fn sweep_edit(
    name: &str,
    fixtures: &[Fixture],
    cases: usize,
    operators: &[(&str, EditOperator)],
) {
    let meshes: Vec<(&str, EditMesh)> = fixtures
        .iter()
        .map(|fixture| (fixture.name.as_str(), fixture.edit_mesh()))
        .collect();
    for (fixture_name, mesh) in &meshes {
        eprintln!(
            "{name}: fixture {fixture_name} with {} faces",
            mesh.face_count()
        );
    }
    let mut sweep = Sweep::new(name);
    for &(operator_name, operator) in operators {
        for (fixture_name, source) in &meshes {
            for index in 0..cases {
                let mut rng = Rng::new(seed(&[operator_name, fixture_name], index));
                let selection = selection(index, &mut rng);
                let label =
                    format!("{operator_name} on {fixture_name} case {index} ({selection:?})");
                let mut error = None;
                sweep.run(&label, || {
                    let mut mesh = source.clone();
                    selection.apply(&mut mesh, &mut rng);
                    let finite = is_finite(&mesh);
                    match operator(&mut mesh, &mut rng) {
                        Ok(()) => check(&mesh, finite),
                        Err(message) => {
                            error = Some(message);
                            Ok(())
                        }
                    }
                });
                if let Some(message) = error {
                    sweep.note(format!("{operator_name}: {message}"));
                }
            }
        }
    }
    sweep.finish();
}

struct ChainStep<'a> {
    name: &'a str,
    operator: EditOperator,
    selection: Option<Selection>,
}

fn next_step<'a>(
    rng: &mut Rng,
    operators: &[(&'a str, EditOperator)],
    first: bool,
) -> ChainStep<'a> {
    let (name, operator) = operators[rng.below(operators.len())];
    let selection = match first || rng.flip() {
        true => Some(Selection::random(rng)),
        false => None,
    };
    ChainStep {
        name,
        operator,
        selection,
    }
}

fn run_step(
    mesh: &mut EditMesh,
    rng: &mut Rng,
    step: &ChainStep<'_>,
) -> Result<Option<String>, String> {
    if let Some(selection) = step.selection {
        selection.apply(mesh, rng);
    }
    let before = mesh.clone();
    let finite = is_finite(mesh);
    match (step.operator)(mesh, rng) {
        Ok(()) => check(mesh, finite).map(|()| None),
        Err(message) => {
            *mesh = before;
            Ok(Some(message))
        }
    }
}

pub fn sweep_chains(
    name: &str,
    fixtures: &[Fixture],
    chains: usize,
    length: usize,
    operators: &[(&str, EditOperator)],
) {
    let mut sweep = Sweep::new(name);
    for fixture in fixtures {
        let source = fixture.edit_mesh();
        for chain in 0..chains {
            let mut rng = Rng::new(seed(&[name, &fixture.name], chain));
            let mut mesh = source.clone();
            let mut steps: Vec<String> = Vec::new();
            for index in 0..length {
                let step = next_step(&mut rng, operators, index == 0);
                match step.selection {
                    Some(selection) => steps.push(format!("{selection:?} {}", step.name)),
                    None => steps.push(step.name.to_owned()),
                }
                let label = format!(
                    "chain {chain} step {index} on {}: {}",
                    fixture.name,
                    steps.join(" > ")
                );
                let mut error = None;
                let passed = sweep.run(&label, || {
                    error = run_step(&mut mesh, &mut rng, &step)?;
                    Ok(())
                });
                if let Some(message) = error {
                    sweep.note(format!("{}: {message}", step.name));
                }
                if !passed {
                    break;
                }
            }
        }
    }
    sweep.finish();
}

#[derive(Debug)]
pub struct Replay {
    pub mesh: EditMesh,
    pub rng: Rng,
    pub operator: EditOperator,
}

pub fn replay_chain(
    name: &str,
    fixture: &Fixture,
    chain: usize,
    step: usize,
    operators: &[(&str, EditOperator)],
) -> Replay {
    let mut rng = Rng::new(seed(&[name, &fixture.name], chain));
    let mut mesh = fixture.edit_mesh();
    for index in 0..step {
        let current = next_step(&mut rng, operators, index == 0);
        if let Err(error) = run_step(&mut mesh, &mut rng, &current) {
            panic!("replayed step {index} failed: {error}");
        }
    }
    let last = next_step(&mut rng, operators, step == 0);
    if let Some(selection) = last.selection {
        selection.apply(&mut mesh, &mut rng);
    }
    Replay {
        mesh,
        rng,
        operator: last.operator,
    }
}
