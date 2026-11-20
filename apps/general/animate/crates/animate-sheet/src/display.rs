use animate_motion::Part;
use rikka_mmd::{Pmx, PmxDisplayTarget};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BoneGroup {
    Frame(String),
    Region(Part),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoneDisplay {
    groups: Vec<(BoneGroup, Vec<String>)>,
    bones: Vec<String>,
    switches: bool,
}

impl BoneDisplay {
    #[must_use]
    pub fn from_pmx(pmx: &Pmx) -> Self {
        let groups = pmx
            .display_frames
            .iter()
            .map(|frame| {
                let bones = frame
                    .targets
                    .iter()
                    .filter_map(|target| match *target {
                        PmxDisplayTarget::Bone(index) => {
                            pmx.bones.get(index).map(|bone| bone.name.clone())
                        }
                        PmxDisplayTarget::Morph(_) => None,
                    })
                    .collect();
                (BoneGroup::Frame(frame.name.clone()), bones)
            })
            .collect();
        Self {
            groups,
            bones: pmx.bones.iter().map(|bone| bone.name.clone()).collect(),
            switches: true,
        }
    }

    #[must_use]
    pub fn by_region(bones: impl IntoIterator<Item = (String, Part)>) -> Self {
        let mut display = Self::default();
        for (bone, part) in bones {
            match display
                .groups
                .iter_mut()
                .find(|(group, _)| *group == BoneGroup::Region(part))
            {
                Some((_, members)) => members.push(bone.clone()),
                None => display
                    .groups
                    .push((BoneGroup::Region(part), vec![bone.clone()])),
            }
            display.bones.push(bone);
        }
        display.groups.sort_by_key(|(group, _)| match group {
            BoneGroup::Region(part) => part.index(),
            BoneGroup::Frame(_) => 0,
        });
        display
    }

    pub(crate) fn groups(&self) -> &[(BoneGroup, Vec<String>)] {
        &self.groups
    }

    pub(crate) fn bones(&self) -> &[String] {
        &self.bones
    }

    pub(crate) fn has_switches(&self) -> bool {
        self.switches
    }
}
