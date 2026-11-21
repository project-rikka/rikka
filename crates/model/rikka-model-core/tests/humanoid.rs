use rikka_model_core::{HumanBone, Side};

#[test]
fn sided_bones_mirror_onto_their_opposite() {
    for &bone in HumanBone::ALL {
        let mirrored = bone.mirrored();
        assert_eq!(mirrored.side(), bone.side().opposite(), "{bone}");
        assert_eq!(mirrored.mirrored(), bone, "{bone}");
        assert_eq!(bone.side() == Side::Center, mirrored == bone, "{bone}");
        assert_eq!(
            bone.parent().map(HumanBone::mirrored),
            mirrored.parent(),
            "{bone}"
        );
    }
}

#[test]
fn every_bone_reaches_the_hips() {
    for &bone in HumanBone::ALL {
        let mut current = bone;
        let mut steps = 0;
        while let Some(parent) = current.parent() {
            current = parent;
            steps += 1;
            assert!(steps <= HumanBone::ALL.len(), "{bone} loops");
        }
        assert_eq!(current, HumanBone::Hips, "{bone}");
    }
}
