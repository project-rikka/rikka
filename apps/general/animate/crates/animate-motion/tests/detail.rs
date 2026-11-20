use std::sync::Arc;

use animate_motion::{DetailPlayback, Take};
use rikka_mmd::FRAMES_PER_SECOND;
use rikka_model_core::Instance;

use crate::support::{MODEL, dance, miku};

const BAKED_DRIFT: f32 = 5e-4;

#[test]
fn detail_playback_poses_bones_like_the_vmd_animation() {
    let model = &*MODEL;
    let motion = dance();
    let take = Take::from_vmd(miku(false), motion.clone());
    assert!(take.human.is_none());
    let playback = DetailPlayback::new(model, Arc::new(take));
    let animation = motion.animation(model);
    let (mut played, mut expected) = (
        Instance::new(Arc::clone(model)),
        Instance::new(Arc::clone(model)),
    );
    for frame in (0..=motion.last_frame()).step_by(29) {
        let seconds = frame as f32 / FRAMES_PER_SECOND;
        played.pose_mut().reset(model);
        expected.pose_mut().reset(model);
        playback.write(seconds, played.pose_mut());
        animation.apply(seconds, expected.pose_mut());
        for (index, node) in model.nodes.iter().enumerate() {
            let id = rikka_model_core::NodeId(index as u32);
            let (one, two) = (played.pose().local(id), expected.pose().local(id));
            assert!(
                one.translation.distance(two.translation)
                    <= BAKED_DRIFT * (1.0 + two.translation.length()),
                "{} moves apart at frame {frame}",
                node.name
            );
            assert!(
                one.rotation.dot(two.rotation).abs() > 1.0 - 1e-5,
                "{} turns apart at frame {frame}",
                node.name
            );
        }
        assert!(playback.visible(seconds));
    }
}
