use std::f32::consts::TAU;
use std::sync::Arc;

use crate::support::{Curtain, Look};
use glam::{Mat4, Vec3};
use rikka_model_core::Instance;
use rikka_physics::{
    BakeView, EmitterShape, EmitterState, FigureKey, FigureMotion, FluidBuilder, Launch, Layout,
    LiquidEmitter, Playback, SceneFrame, SceneSource,
};
use rikka_physics_liquid::Preset;
use rikka_physics_tailor::{Grab, Wind};

const LAST: u32 = 30;
const CURTAIN: FigureKey = FigureKey(2);

struct Sway {
    layout: Layout,
    emitters: Vec<LiquidEmitter>,
    rates: Vec<(u32, f32)>,
}

impl Sway {
    fn pouring(curtain: &Curtain, rates: &[(u32, f32)]) -> Arc<dyn SceneSource> {
        let fluid = FluidBuilder::default()
            .radius(0.006)
            .capacity(20_000)
            .floor(0.0);
        Arc::new(Sway {
            layout: Layout::new(fluid).drying(1.0 / 30.0).figure_with_cloth(
                CURTAIN,
                Arc::clone(&curtain.model),
                Arc::clone(&curtain.asset),
            ),
            emitters: vec![LiquidEmitter {
                id: 1,
                liquid: Preset::Water.liquid(),
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

impl SceneSource for Sway {
    fn layout(&self) -> &Layout {
        &self.layout
    }

    fn emitters(&self) -> &[LiquidEmitter] {
        &self.emitters
    }

    fn pose(&self, _: usize, time: f32, instance: &mut Instance) -> FigureMotion {
        let sway = 0.08 * (TAU * time).sin();
        instance.set_transform(Mat4::from_translation(Vec3::new(sway, 0.0, 0.0)));
        FigureMotion::Moving
    }

    fn emitter_states(&self, step: u32, _: &[Instance]) -> Vec<EmitterState> {
        vec![EmitterState {
            id: 1,
            shape: EmitterShape::Stream { radius: 0.02 },
            enabled: true,
            rate: self.rate_at(step),
            volume: 0.0,
            launch: Some(Launch {
                position: Vec3::new(0.0, 0.55, 0.25),
                velocity: Some(Vec3::new(0.0, 0.0, -1.5)),
            }),
            bursts: 0,
        }]
    }

    fn wind(&self, time: f32) -> Wind {
        Wind {
            direction: Vec3::X,
            speed: 1.0 + time,
            turbulence: 0.3,
            gust: 0.2,
        }
    }
}

fn hung() -> Curtain {
    crate::support::curtain(8, 8, 0.04, 0.6)
}

#[test]
fn cloth_bake_matches_direct_run_and_recomputes_edits() {
    let compute = crate::support::compute();
    let (bake, notify) = crate::support::start(&compute);
    bake.set_target_step(LAST);
    let curtain = hung();
    let steady = Sway::pouring(&curtain, &[(0, 0.2e-3)]);
    bake.set_source(Arc::clone(&steady), Some(0));
    crate::support::baked(&bake, &notify, LAST);

    let look = |scene_frame: &SceneFrame| crate::support::look(&compute, scene_frame, CURTAIN);
    let mut display = BakeView::default();
    let steps = [4, 17, LAST];
    let viewed: Vec<Look> = steps
        .iter()
        .map(|step| look(&crate::support::shown(&compute, &bake, &mut display, *step)))
        .collect();
    assert!(
        viewed
            .iter()
            .all(|scene_frame| scene_frame.cloth.len() == 1),
        "the curtain is draped"
    );
    assert!(
        viewed.windows(2).all(|pair| pair[0].cloth != pair[1].cloth),
        "the curtain sways"
    );
    assert!(
        viewed
            .windows(2)
            .all(|pair| pair[0].particles.len() < pair[1].particles.len()),
        "the stream keeps pouring"
    );
    assert_eq!(
        viewed,
        crate::support::simulated(&compute, &steady, &steps, look)
    );

    let faster = Sway::pouring(&curtain, &[(0, 0.2e-3), (20, 0.6e-3)]);
    bake.set_source(Arc::clone(&faster), Some(20));
    crate::support::baked(&bake, &notify, LAST);
    let early = look(&crate::support::shown(&compute, &bake, &mut display, 17));
    let late = look(&crate::support::shown(&compute, &bake, &mut display, LAST));
    assert_eq!(early, viewed[1], "steps before the edit are kept");
    assert!(
        late.particles.len() > viewed[2].particles.len(),
        "the edit pours faster"
    );
    assert_eq!(
        vec![early, late],
        crate::support::simulated(&compute, &faster, &[17, LAST], look)
    );
}

#[test]
fn simulated_playback_matches_baked() {
    let compute = crate::support::compute();
    let (bake, notify) = crate::support::start(&compute);
    bake.set_target_step(LAST);
    let curtain = hung();
    let source = Sway::pouring(&curtain, &[(0, 0.2e-3)]);
    bake.set_source(Arc::clone(&source), Some(0));
    crate::support::baked(&bake, &notify, LAST);

    let mut viewport = Playback::baked(bake);
    let mut export = Playback::simulated();
    for step in 0..=LAST {
        let observe = |playback: &mut Playback| {
            let scene_frame = playback
                .frame(&compute, &source, step)
                .expect("simulated frame")
                .expect("a liquid to draw");
            crate::support::look(&compute, &scene_frame, CURTAIN)
        };
        assert_eq!(observe(&mut viewport), observe(&mut export), "step {step}");
    }
}

struct Splash {
    layout: Layout,
    emitters: Vec<LiquidEmitter>,
    volume: f32,
    hand: Option<Grab>,
}

impl Splash {
    fn thrown(curtain: &Curtain, volume: f32) -> Arc<dyn SceneSource> {
        Self::grabbed(curtain, volume, None)
    }

    fn grabbed(curtain: &Curtain, volume: f32, hand: Option<Grab>) -> Arc<dyn SceneSource> {
        let fluid = FluidBuilder::default()
            .radius(0.004)
            .capacity(20_000)
            .floor(0.0);
        Arc::new(Splash {
            layout: Layout::new(fluid).figure_with_cloth(
                CURTAIN,
                Arc::clone(&curtain.model),
                Arc::clone(&curtain.asset),
            ),
            emitters: vec![LiquidEmitter {
                id: 1,
                liquid: Preset::Water.liquid(),
                region: None,
            }],
            volume,
            hand,
        })
    }
}

impl SceneSource for Splash {
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
            shape: EmitterShape::Blob,
            enabled: true,
            rate: 0.0,
            volume: self.volume,
            launch: Some(Launch {
                position: Vec3::new(0.0, 0.5, 0.15),
                velocity: Some(Vec3::new(0.0, 0.0, -2.0)),
            }),
            bursts: u32::from(step == 0 && self.volume > 0.0),
        }]
    }

    fn wind(&self, _: f32) -> Wind {
        Wind::CALM
    }

    fn grabs(&self, _: f32, _: &[Instance]) -> Vec<(usize, Grab)> {
        self.hand.iter().map(|grab| (0, grab.clone())).collect()
    }
}

fn hung_after(
    compute: &rikka_gpu::Compute,
    source: &Arc<dyn SceneSource>,
    steps: u32,
) -> Vec<Vec3> {
    let mut frames = crate::support::simulated(compute, source, &[steps], |scene_frame| {
        crate::support::drapes(compute, scene_frame.cloth(CURTAIN))
    });
    let drapes = frames.pop().expect("simulated frame");
    assert_eq!(drapes.len(), 1, "the curtain is draped");
    crate::support::positions(&drapes[0])
}

#[test]
fn thrown_blob_pushes_curtain() {
    let compute = crate::support::compute();
    let curtain = crate::support::curtain(6, 6, 0.04, 0.6);
    let dry = hung_after(&compute, &Splash::thrown(&curtain, 0.0), 20);
    let wet = hung_after(&compute, &Splash::thrown(&curtain, 3.0e-5), 20);
    let depth = |positions: &[Vec3]| {
        positions.iter().map(|position| position.z).sum::<f32>() / positions.len() as f32
    };
    let farthest = dry
        .iter()
        .zip(&wet)
        .map(|(dry, wet)| dry.distance(*wet))
        .fold(0.0, f32::max);
    assert!(
        farthest > 0.005,
        "the blob moves the curtain by {farthest} m"
    );
    assert!(
        depth(&wet) < depth(&dry),
        "the blob pushes the curtain back: {} against {}",
        depth(&wet),
        depth(&dry)
    );
}

#[test]
fn source_grab_lifts_curtain() {
    let compute = crate::support::compute();
    let curtain = crate::support::curtain(6, 6, 0.04, 0.6);
    let hem = curtain.asset.pieces[0]
        .particles
        .iter()
        .filter(|particle| !particle.pinned)
        .map(|particle| particle.rest)
        .min_by(|one, two| one.y.total_cmp(&two.y))
        .expect("the curtain hangs loose");
    let pick = curtain
        .asset
        .pick(hem, 0.05)
        .expect("the hem lies under the hand");
    let hand = Grab {
        id: 1,
        pick: Arc::new(pick),
        point: hem + Vec3::new(0.0, 0.1, 0.15),
        strength: 1.0,
    };
    let free = hung_after(&compute, &Splash::thrown(&curtain, 0.0), 20);
    let grabbed = hung_after(&compute, &Splash::grabbed(&curtain, 0.0, Some(hand)), 20);
    let forward = |positions: &[Vec3]| positions.iter().map(|at| at.z).fold(f32::MIN, f32::max);
    assert!(
        forward(&grabbed) > forward(&free) + 0.1,
        "the hand lifts the hem forward: {} against {}",
        forward(&grabbed),
        forward(&free)
    );
}
