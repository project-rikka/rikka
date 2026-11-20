use std::sync::Arc;

use crate::support::{sample, sheet_asset};
use glam::{Mat4, Vec3};
use rikka_model_core::{Instance, Model};
use rikka_physics_cloth::{Cloth, FigureId, Inputs, Traced};
use rikka_physics_tailor::{ClothAsset, Curve, PieceKind, Style};

const RUNS: usize = 3;
const APART: f32 = 25.0;

struct Scene {
    builder: fn() -> Cloth,
    assets: Vec<(Arc<ClothAsset>, Option<Style>)>,
    frames: u32,
    pose: Box<dyn Fn(u32, usize) -> Instance>,
}

type Steps = Vec<Vec<Traced>>;

fn run(scene: &Scene, decoy: bool) -> Steps {
    let mut cloth = (scene.builder)();
    cloth.set_tracing(true);
    let mut figures: Vec<FigureId> = scene
        .assets
        .iter()
        .map(|(asset, style)| match style {
            Some(style) => cloth.add_styled_figure(Arc::clone(asset), [style.clone()]),
            None => cloth.add_figure(Arc::clone(asset)),
        })
        .collect::<Result<_, _>>()
        .expect("figures");
    let primaries = figures.len();
    if decoy {
        let (asset, style) = &scene.assets[0];
        figures.push(
            match style {
                Some(style) => cloth.add_styled_figure(Arc::clone(asset), [style.clone()]),
                None => cloth.add_figure(Arc::clone(asset)),
            }
            .expect("a decoy"),
        );
    }
    let mut steps = Vec::new();
    for frame in 0..scene.frames {
        let instances: Vec<Instance> = (0..figures.len())
            .map(|index| {
                let mut instance = (scene.pose)(frame, index.min(primaries - 1));
                if index >= primaries {
                    let moved =
                        Mat4::from_translation(Vec3::new(APART, 0.0, APART)) * instance.transform();
                    instance.set_transform(moved);
                }
                instance
            })
            .collect();
        let inputs = figures
            .iter()
            .zip(&instances)
            .fold(Inputs::new(), |inputs, (&figure, instance)| {
                inputs.figure(figure, instance)
            });
        cloth
            .advance(f64::from(frame) / f64::from(cloth.rate()), &inputs)
            .expect("a step");
        let traced = cloth.trace().expect("a trace");
        steps.push(
            traced
                .into_iter()
                .filter(|traced| {
                    traced
                        .figure
                        .is_some_and(|figure| figures[..primaries].contains(&figure))
                })
                .collect(),
        );
    }
    steps
}

fn first_difference(expected: &Steps, actual: &Steps) -> Option<String> {
    for (step, (left, right)) in expected.iter().zip(actual).enumerate() {
        if left.len() != right.len() {
            return Some(format!(
                "step {step} traced {} buffers, then {}",
                left.len(),
                right.len()
            ));
        }
        if let Some((was, now)) = left.iter().zip(right).find(|(was, now)| was != now) {
            return Some(format!(
                "step {step}, round {} epoch {:?}: {:?} {} after {} first differs ({:016x} then {:016x})",
                was.round, was.epoch, was.figure, was.buffer, was.pass, was.hash, now.hash
            ));
        }
    }
    None
}

fn reproduces(name: &str, scene: &Scene) {
    let expected = run(scene, false);
    assert!(expected.iter().all(|step| !step.is_empty()));
    let mut differences = Vec::new();
    for attempt in 0..RUNS {
        for decoy in [false, true] {
            if attempt == 0 && !decoy {
                continue;
            }
            if let Some(difference) = first_difference(&expected, &run(scene, decoy)) {
                differences.push(format!("{name} run {attempt}, decoy {decoy}: {difference}"));
            }
        }
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

fn dancing(model: &Arc<Model>, frame: u32, offset: Vec3) -> Instance {
    let mut instance = sample::dance(model);
    instance.seek(frame as f32 / 60.0);
    instance.set_transform(Mat4::from_translation(offset));
    instance
}

#[test]
fn folding_sheet_is_deterministic_across_schedules() {
    let (model, asset) = sheet_asset(14, 20, 0.02, PieceKind::Sheet);
    let style = Style {
        self_collision: true,
        thickness: 0.004,
        damping: Curve::flat(0.05),
        ..crate::support::style()
    };
    let scene = Scene {
        builder: || crate::support::cloth(Cloth::builder().floor(0.0)),
        assets: vec![(asset, Some(style))],
        frames: 150,
        pose: Box::new(move |frame, _| {
            let lowered = (frame as f32 / 60.0).min(1.0);
            let sideways = 0.08 * (frame as f32 / 30.0).sin() * lowered;
            let mut instance = Instance::new(Arc::clone(&model));
            instance.set_transform(Mat4::from_translation(Vec3::new(
                sideways,
                0.42 - 0.38 * lowered,
                0.0,
            )));
            instance
        }),
    };
    reproduces("fold", &scene);
}

#[test]
fn dance_is_deterministic_across_schedules() {
    let model = sample::miku();
    let scene = Scene {
        builder: || crate::support::cloth(Cloth::builder()),
        assets: vec![(sample::asset(&model), None)],
        frames: 90,
        pose: Box::new(move |frame, _| dancing(&model, frame, Vec3::ZERO)),
    };
    reproduces("dance", &scene);
}
