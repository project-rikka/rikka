use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::support::sample;
use rikka_model_core::{Instance, Model};
use rikka_physics_cloth::{Cloth, DRAPE_VERTEX_BYTES, FigureId};
use rikka_physics_tailor::{ClothAsset, Quality};

const DANCED: u32 = 45;
const HELD: u32 = 100;
const CALMED: u32 = 45;
const SLEPT: u32 = 100;
const READ_EVERY_STEP_UNTIL: u32 = DANCED / 2;
const CHECKPOINT_EVERY: u32 = 20;
const PAUSE: Duration = Duration::from_millis(4);
const RESTORED_AT: u32 = DANCED + 20;
const DETOUR: u32 = 12;

struct Ending {
    drapes: Vec<Vec<u8>>,
    particles: Vec<u32>,
    liveliest: f32,
    asleep: Option<u32>,
    motion: f32,
}

fn drape_bytes(cloth: &Cloth, figure: FigureId) -> Vec<Vec<u8>> {
    cloth
        .drapes(figure)
        .expect("drapes")
        .iter()
        .map(|drape| {
            drape.ready.wait().expect("a drape");
            crate::support::read(
                &drape.buffer,
                u64::from(drape.vertices) * DRAPE_VERTEX_BYTES,
            )
        })
        .collect()
}

fn read_along(cloth: &mut Cloth, figure: FigureId, instance: &Instance, frame: u32) {
    match frame {
        RESTORED_AT => {
            let checkpoint = cloth.checkpoint().expect("a checkpoint");
            for detour in frame + 1..=frame + DETOUR {
                crate::support::step(cloth, figure, instance, detour);
                drop(cloth.read_particles(figure).expect("particles"));
            }
            cloth.restore(&checkpoint).expect("a restore");
        }
        paused if (READ_EVERY_STEP_UNTIL..RESTORED_AT).contains(&paused) => {
            thread::sleep(PAUSE);
            if paused.is_multiple_of(CHECKPOINT_EVERY) {
                drop(cloth.checkpoint().expect("a checkpoint"));
            }
        }
        _ => drop(cloth.read_particles(figure).expect("particles")),
    }
}

fn stop_mid_dance(model: &Arc<Model>, asset: &Arc<ClothAsset>, read: bool) -> Ending {
    let mut cloth = crate::support::cloth(Cloth::builder().quality(Quality::Medium).floor(0.0));
    let figure = cloth.add_figure(Arc::clone(asset)).expect("a figure");
    let mut instance = sample::dance(model);
    let mut liveliest = 0.0f32;
    let mut asleep = None;
    for frame in 0..DANCED + HELD {
        if frame < DANCED {
            instance.advance(1.0 / 60.0);
        }
        crate::support::step(&mut cloth, figure, &instance, frame);
        if read {
            read_along(&mut cloth, figure, &instance, frame);
        }
        let since_stop = frame.saturating_sub(DANCED);
        if (CALMED..CALMED + 15).contains(&since_stop) {
            liveliest = liveliest.max(cloth.motion());
        }
        if frame >= DANCED && asleep.is_none() && cloth.last_substeps() == 0 {
            asleep = Some(since_stop);
        }
    }
    Ending {
        drapes: drape_bytes(&cloth, figure),
        particles: crate::support::bits(&mut cloth, figure),
        liveliest,
        asleep,
        motion: cloth.motion(),
    }
}

#[test]
fn cloth_settles_within_two_seconds_after_stop() {
    let model = sample::miku();
    let asset = sample::asset(&model);
    let unread = stop_mid_dance(&model, &asset, false);
    assert!(
        unread
            .particles
            .iter()
            .all(|bits| f32::from_bits(*bits).is_finite())
    );
    assert!(
        unread.liveliest < 0.1,
        "the cloth still moves at {} m/s {CALMED} steps after the figure stopped",
        unread.liveliest
    );
    assert!(
        unread.asleep.is_some_and(|frame| frame < SLEPT),
        "the cloth fell asleep only {:?} steps after the figure stopped",
        unread.asleep
    );
    assert_eq!(unread.motion, 0.0, "the stopped cloth never stayed asleep");

    let read = stop_mid_dance(&model, &asset, true);
    assert!(
        read.drapes == unread.drapes,
        "reading the cloth changed the drapes"
    );
    assert!(
        read.particles == unread.particles,
        "reading the cloth changed the particles"
    );
}
