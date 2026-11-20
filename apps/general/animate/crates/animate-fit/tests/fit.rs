use animate_body::{HumanBone, HumanPose, Limb, Worlds};
use animate_fit::Physique;
use animate_solve::reach;
use rikka_model_core::glam::Vec3;
use rikka_physics_tailor::Preset;

use crate::support::SAMPLE;

const FRAMES: usize = 161;
const HELD_AWAY: usize = 40;
const UPPER_HEIGHT: f32 = 0.45;
const HEM_HEIGHT: f32 = 0.3;
const SLAB: f32 = 0.015;
const ALONG: f32 = 0.97;
const PRESSED_SLACK: f32 = 0.002;

fn reaching(goal: Vec3, away: Vec3) -> Vec<HumanPose> {
    let body = &SAMPLE.body;
    let rest = *body.rest_pose();
    (0..FRAMES)
        .map(|frame| {
            let middle = (FRAMES / 2) as f32;
            let reaching = middle - HELD_AWAY as f32;
            let share = ((frame as f32 - middle).abs() / reaching).min(1.0);
            let mut pose = rest;
            reach(
                body,
                &mut pose,
                Limb::LeftArm,
                goal.lerp(away, share),
                None,
                1.0,
            );
            pose
        })
        .collect()
}

fn hand_inside_chest_pose() -> Vec<HumanPose> {
    let body = &SAMPLE.body;
    let chest = body.fk(body.rest_pose()).position(HumanBone::UpperChest);
    reaching(chest, chest + Vec3::new(0.35, 0.0, 0.45) * body.scale())
}

fn hand(worlds: &Worlds) -> Vec3 {
    worlds.position(HumanBone::LeftHand)
}

fn fitted(physique: &Physique, poses: &[HumanPose]) -> Vec<HumanPose> {
    let unfitted: Vec<(HumanPose, [bool; 2])> =
        poses.iter().map(|pose| (*pose, [false; 2])).collect();
    physique.fit(&SAMPLE.body, 0, &unfitted, 0..poses.len())
}

struct Skirt {
    centre: Vec3,
    toward: Vec3,
    surface: f32,
    skin: f32,
}

impl Skirt {
    fn toward(height: f32, toward: Vec3) -> Self {
        let body = &SAMPLE.body;
        let hips = body.fk(body.rest_pose()).position(HumanBone::Hips);
        let skirt = SAMPLE
            .wearer
            .garments
            .iter()
            .find(|garment| garment.preset == Preset::Skirt)
            .expect("miku wears a skirt");
        let (low, high) = skirt
            .vertices
            .iter()
            .fold((f32::MAX, f32::MIN), |(low, high), vertex| {
                (low.min(vertex.rest.y), high.max(vertex.rest.y))
            });
        let centre = Vec3::new(hips.x, low + height * (high - low), hips.z);
        let slab = SLAB * body.scale();
        let radial = |points: &mut dyn Iterator<Item = Vec3>| -> Vec<f32> {
            points
                .filter(|point| (point.y - centre.y).abs() < slab)
                .filter(|point| (*point - centre).normalize().dot(toward) > ALONG)
                .map(|point| (point - centre).dot(toward))
                .collect()
        };
        let surface = radial(&mut skirt.vertices.iter().map(|vertex| vertex.rest))
            .into_iter()
            .fold(f32::MIN, f32::max);
        let skin = radial(&mut SAMPLE.wearer.body.vertices.iter().map(|vertex| vertex.rest))
            .into_iter()
            .filter(|radius| *radius < surface)
            .fold(0.0, f32::max);
        Self {
            centre,
            toward,
            surface,
            skin,
        }
    }

    fn room(&self) -> f32 {
        self.surface - self.skin
    }

    fn reaching_in(&self, depth: f32) -> Vec<HumanPose> {
        let goal = self.centre + self.toward * (self.surface - depth);
        reaching(goal, goal + self.toward * 0.25 * SAMPLE.body.scale())
    }

    fn depth(&self, pose: &HumanPose) -> f32 {
        self.surface - (hand(&SAMPLE.body.fk(pose)) - self.centre).dot(self.toward)
    }
}

#[test]
fn fit_pushes_hand_out_of_chest() {
    let body = &SAMPLE.body;
    let scale = body.scale();
    let before = hand_inside_chest_pose();
    let after = fitted(&SAMPLE.physique, &before);
    assert_eq!(
        after[0], before[0],
        "a hand far from the body is left alone"
    );

    let deepest = FRAMES / 2;
    let (was, now) = (body.fk(&before[deepest]), body.fk(&after[deepest]));
    let bulk = body.bulk();
    assert!(
        bulk.holds(&was, hand(&was), -0.03 * scale),
        "the hand starts inside"
    );
    assert!(
        !bulk.holds(&now, hand(&now), -0.01 * scale),
        "the hand is pulled out"
    );
    let chest = now.position(HumanBone::UpperChest);
    let facing = now.rotation(HumanBone::Hips) * Vec3::Z;
    assert!(
        (hand(&now) - chest).dot(facing) > 0.0,
        "the hand leaves through the front"
    );

    let jolt = |poses: &[HumanPose]| {
        let path: Vec<Vec3> = poses.iter().map(|pose| hand(&body.fk(pose))).collect();
        path.windows(3)
            .map(|three| (three[0] - 2.0 * three[1] + three[2]).length())
            .fold(0.0, f32::max)
    };
    assert!(
        jolt(&after) < 1.5 * jolt(&before),
        "the correction eases in and out without a jolt: {} against {}",
        jolt(&after),
        jolt(&before)
    );
}

fn beside_left_thigh(height: f32) -> Skirt {
    Skirt::toward(height, Vec3::X)
}

#[test]
fn fit_draws_hand_out_of_skirt() {
    let skirt = beside_left_thigh(UPPER_HEIGHT);
    let room = skirt.room();
    assert!(
        room > 0.04 * SAMPLE.body.scale(),
        "the skirt stands off the thigh: {room}"
    );
    let before = skirt.reaching_in(0.8 * room);
    let deepest = FRAMES / 2;
    assert!(
        skirt.depth(&before[deepest]) > 0.7 * room,
        "the hand starts deep inside the skirt"
    );

    let bare = fitted(&SAMPLE.bare, &before);
    let pressed = skirt.depth(&bare[deepest]);
    assert!(
        pressed > 0.5 * room,
        "the body alone leaves the hand deep inside the skirt: {pressed} of {room}"
    );

    let after = fitted(&SAMPLE.physique, &before);
    assert_eq!(
        after[0], before[0],
        "a hand clear of the skirt is left alone"
    );
    let drawn = skirt.depth(&after[deepest]);
    assert!(
        drawn < 0.5 * pressed,
        "the skirt draws the hand out to where it gives way: {drawn} against {pressed}"
    );
}

#[test]
fn fit_lets_hand_press_skirt_where_it_gives_way() {
    let body = &SAMPLE.body;
    let skirt = beside_left_thigh(HEM_HEIGHT);
    let before = skirt.reaching_in(0.3 * skirt.room());
    let moved = |physique: &Physique| {
        before
            .iter()
            .zip(fitted(physique, &before))
            .map(|(was, now)| hand(&body.fk(was)).distance(hand(&body.fk(&now))))
            .fold(0.0, f32::max)
    };
    let (dressed, bare) = (moved(&SAMPLE.physique), moved(&SAMPLE.bare));
    assert!(
        dressed < bare + PRESSED_SLACK * body.scale(),
        "the skirt leaves the pressing hand in place: moved {dressed} against {bare}"
    );
}
