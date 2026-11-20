use animate_body::Side;
use animate_hold::{
    Hold, HoldTarget, Holder, Holds, Performer, Pinches, active_grabs, grip_points,
};
use rikka_mmd::FRAMES_PER_SECOND;
use rikka_model_core::Instance;
use rikka_model_core::glam::Vec3;

use crate::support::{Sample, miku, standing};

const REACH: f32 = 0.05;

fn hem(sample: &Sample) -> Vec3 {
    sample
        .cloth_asset
        .pieces
        .iter()
        .flat_map(|piece| piece.particles.iter())
        .filter(|particle| !particle.pinned)
        .max_by(|one, two| one.reach.total_cmp(&two.reach))
        .map(|particle| particle.rest)
        .expect("the sample wears loose cloth")
}

fn holding(figure: Option<&str>, at: Vec3, from: u32, until: Option<u32>) -> Holds {
    let mut holds = Holds::default();
    holds.lay(Hold {
        until,
        ..Hold::new(
            Holder::Hand(Side::Right),
            HoldTarget {
                figure: figure.map(str::to_owned),
                at,
                reach: REACH,
            },
            from,
        )
    });
    holds
}

fn performer<'a>(
    sample: &'a Sample,
    key: u64,
    instance: &'a Instance,
    holds: Option<&'a Holds>,
) -> Performer<'a> {
    Performer {
        key,
        name: "Miku",
        instance,
        pinches: Pinches::new(Some(&sample.body), &sample.model),
        holds,
        cloth_asset: Some(&sample.cloth_asset),
    }
}

fn seconds(frame: u32) -> f32 {
    frame as f32 / FRAMES_PER_SECOND
}

#[test]
fn hand_grips_own_cloth_at_pinch() {
    let sample = miku();
    let instance = standing(sample);
    let holds = holding(None, hem(sample), 10, Some(40));
    let performers = [performer(sample, 1, &instance, Some(&holds))];
    let pinch = performers[0]
        .pinches
        .point(Side::Right, &instance)
        .expect("the sample has a right hand");
    let rest = sample.body.pinch(Side::Right).expect("a pinch").rest;
    assert!(
        pinch.distance(rest) < 1.0e-3,
        "{pinch} sits at the rest pinch {rest}"
    );

    let grabs = active_grabs(seconds(20), &performers);
    assert_eq!(grabs.len(), 1);
    assert_eq!(grabs[0].owner, 0);
    assert_eq!(grabs[0].grab.point, pinch);
    assert_eq!(grabs[0].grab.strength, 1.0);
    assert!(!grabs[0].grab.pick.members.is_empty());
    assert!(
        active_grabs(seconds(60), &performers).is_empty(),
        "the hand let go"
    );

    let target = grip_points(seconds(20), &performers, 0);
    assert_eq!(target.len(), 1);
    assert_eq!(target[0].point, pinch);
    assert!(grip_points(seconds(45), &performers, 0).is_empty());
}

#[test]
fn hand_resolves_held_figure_by_name() {
    let sample = miku();
    let instance = standing(sample);
    let holds = holding(Some("Miku"), hem(sample), 0, None);
    let performers = [
        performer(sample, 1, &instance, Some(&holds)),
        performer(sample, 2, &instance, None),
    ];
    let grabs = active_grabs(seconds(5), &performers);
    assert_eq!(grabs.len(), 1);
    assert_eq!(grabs[0].owner, 1);

    let strangers = holding(Some("Rin"), hem(sample), 0, None);
    let alone = [performer(sample, 1, &instance, Some(&strangers))];
    assert!(active_grabs(seconds(5), &alone).is_empty());
}

#[test]
fn pin_grips_at_bone_offset() {
    let sample = miku();
    let instance = standing(sample);
    let at = Vec3::new(0.0, 0.5, 0.3);
    let mut holds = Holds::default();
    holds.lay(Hold {
        ease: 0,
        ..Hold::new(
            Holder::Pin { node: 0, at },
            HoldTarget {
                figure: None,
                at: hem(sample),
                reach: REACH,
            },
            0,
        )
    });
    let performers = [performer(sample, 7, &instance, Some(&holds))];
    let grabs = active_grabs(seconds(3), &performers);
    assert_eq!(grabs.len(), 1);
    assert_eq!(grabs[0].grab.strength, 1.0);
    assert!(grabs[0].grab.point.distance(at) < 1.0e-4);
}
