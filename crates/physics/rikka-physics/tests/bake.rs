use std::sync::Arc;

use glam::Vec3;
use rikka_model_core::{Instance, Model};
use rikka_physics::{
    BakeView, EmitterShape, EmitterState, FigureKey, FigureMotion, FluidBuilder, Launch, Layout,
    LiquidEmitter, SceneSource,
};
use rikka_physics_liquid::Preset;
use rikka_physics_tailor::Wind;

const LAST: u32 = 30;
const TIGHT_BUDGET: usize = 64 << 10;
const PLANK: FigureKey = FigureKey(1);

struct Hose {
    layout: Layout,
    emitters: Vec<LiquidEmitter>,
    rates: Vec<(u32, f32)>,
}

impl Hose {
    fn pouring(model: &Arc<Model>, liquid: Preset, rates: &[(u32, f32)]) -> Arc<dyn SceneSource> {
        let fluid = FluidBuilder::default()
            .radius(0.006)
            .capacity(20_000)
            .floor(0.0);
        Arc::new(Hose {
            layout: Layout::new(fluid)
                .drying(1.0 / 30.0)
                .figure(PLANK, Arc::clone(model)),
            emitters: vec![LiquidEmitter {
                id: 1,
                liquid: liquid.liquid(),
                region: None,
            }],
            rates: rates.to_vec(),
        })
    }

    fn rate_at(&self, step: u32) -> f32 {
        self.rates
            .iter()
            .rev()
            .find(|(from, _)| *from <= step)
            .map_or(0.0, |(_, rate)| *rate)
    }
}

impl SceneSource for Hose {
    fn layout(&self) -> &Layout {
        &self.layout
    }

    fn emitters(&self) -> &[LiquidEmitter] {
        &self.emitters
    }

    fn pose(&self, _: usize, _: f32, _: &mut Instance) -> FigureMotion {
        FigureMotion::Resting
    }

    fn emitter_states(&self, step: u32, _: &[Instance]) -> Vec<EmitterState> {
        vec![EmitterState {
            id: 1,
            shape: EmitterShape::Stream { radius: 0.02 },
            enabled: true,
            rate: self.rate_at(step),
            volume: 0.0,
            launch: Some(Launch {
                position: Vec3::new(0.0, 0.6, 0.0),
                velocity: Some(Vec3::new(0.2, -0.5, 0.0)),
            }),
            bursts: 0,
        }]
    }

    fn wind(&self, _: f32) -> Wind {
        Wind::CALM
    }
}

#[test]
fn bake_matches_direct_run_and_recomputes_edits() {
    let compute = crate::support::compute();
    let (bake, notify) = crate::support::start(&compute);
    bake.set_target_step(LAST);
    let model = crate::support::plank();
    let steady = Hose::pouring(&model, Preset::Water, &[(0, 0.2e-3)]);
    bake.set_source(Arc::clone(&steady), Some(0));
    crate::support::baked(&bake, &notify, LAST);

    let mut display = BakeView::default();
    let steps = [4, 17, LAST];
    let viewed: Vec<Vec<u8>> = steps
        .iter()
        .map(|step| {
            crate::support::particles(
                &compute,
                &crate::support::shown(&compute, &bake, &mut display, *step),
            )
        })
        .collect();
    assert!(
        viewed.windows(2).all(|pair| pair[0].len() < pair[1].len()),
        "the stream keeps pouring: {:?}",
        viewed.iter().map(Vec::len).collect::<Vec<_>>()
    );
    let particles =
        |scene_frame: &rikka_physics::SceneFrame| crate::support::particles(&compute, scene_frame);
    assert_eq!(
        viewed,
        crate::support::simulated(&compute, &steady, &steps, particles)
    );

    let faster = Hose::pouring(&model, Preset::Water, &[(0, 0.2e-3), (20, 0.6e-3)]);
    bake.set_source(Arc::clone(&faster), Some(20));
    crate::support::baked(&bake, &notify, LAST);
    let early = crate::support::particles(
        &compute,
        &crate::support::shown(&compute, &bake, &mut display, 17),
    );
    let late = crate::support::particles(
        &compute,
        &crate::support::shown(&compute, &bake, &mut display, LAST),
    );
    assert_eq!(early, viewed[1], "steps before the edit are kept");
    assert!(late.len() > viewed[2].len(), "the edit pours faster");
    assert_eq!(
        vec![early, late],
        crate::support::simulated(&compute, &faster, &[17, LAST], particles)
    );
}

#[test]
fn liquid_change_before_first_step_applies() {
    let compute = crate::support::compute();
    let (bake, notify) = crate::support::start(&compute);
    bake.set_target_step(LAST);
    bake.pause(true);
    let model = crate::support::plank();
    let water = Hose::pouring(&model, Preset::Water, &[(0, 0.2e-3)]);
    bake.set_source(Arc::clone(&water), Some(0));
    crate::support::settled(&bake, &notify, |progress| !progress.working);
    let honey = Hose::pouring(&model, Preset::Honey, &[(0, 0.2e-3)]);
    bake.set_source(Arc::clone(&honey), Some(0));
    crate::support::settled(&bake, &notify, |progress| !progress.working);
    bake.pause(false);
    crate::support::baked(&bake, &notify, LAST);
    let shown = crate::support::particles(
        &compute,
        &crate::support::shown(&compute, &bake, &mut BakeView::default(), LAST),
    );
    let particles =
        |scene_frame: &rikka_physics::SceneFrame| crate::support::particles(&compute, scene_frame);
    let poured = crate::support::simulated(&compute, &honey, &[LAST], particles);
    assert_ne!(
        poured,
        crate::support::simulated(&compute, &water, &[LAST], particles),
        "honey and water draw_fluid apart"
    );
    assert_eq!(vec![shown], poured);
}

#[test]
fn tight_budget_bakes_around_viewed_step() {
    let compute = crate::support::compute();
    let (bake, notify) = crate::support::start_within(&compute, TIGHT_BUDGET);
    bake.set_target_step(LAST);
    let model = crate::support::plank();
    let steady = Hose::pouring(&model, Preset::Water, &[(0, 0.2e-3)]);
    bake.set_source(Arc::clone(&steady), Some(0));
    crate::support::settled(&bake, &notify, |progress| !progress.working);
    let cached = bake.progress().cached;
    let [held] = cached.as_slice() else {
        panic!("the first steps are cached in one run: {cached:?}");
    };
    let beyond = held.end() + 1;
    assert!(
        *held.start() == 0 && (5..LAST).contains(&beyond),
        "the budget holds the first steps but not all: {held:?}"
    );

    let mut display = BakeView::default();
    let steps = [4, beyond, LAST, 4];
    let viewed: Vec<Vec<u8>> = steps
        .iter()
        .map(|step| {
            display
                .frame(&bake, &compute, *step)
                .expect("the display mirrors the bake");
            crate::support::settled(&bake, &notify, |progress| !progress.working);
            let shown = crate::support::particles(
                &compute,
                &crate::support::shown(&compute, &bake, &mut display, *step),
            );
            assert!(!display.is_stale(&bake, *step), "step {step} stays shown");
            shown
        })
        .collect();
    let particles =
        |scene_frame: &rikka_physics::SceneFrame| crate::support::particles(&compute, scene_frame);
    let direct = crate::support::simulated(&compute, &steady, &[4, beyond, LAST], particles);
    assert_eq!(viewed[..3], direct[..]);
    assert_eq!(viewed[3], direct[0]);
}
