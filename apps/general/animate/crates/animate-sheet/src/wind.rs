use animate_wind::{WindMotion, WindSetting};

use crate::sheet::{Group, GroupKind, Lane, Sheet, Track};

impl Sheet {
    #[must_use]
    pub fn wind(motion: &WindMotion) -> Self {
        let tracks = WindSetting::ALL
            .iter()
            .map(|setting| {
                Track::plain(
                    Lane::Wind(*setting),
                    motion
                        .tracks()
                        .keys(setting)
                        .iter()
                        .map(|key| key.frame)
                        .collect(),
                )
            })
            .collect();
        Self::new(vec![Group::new(GroupKind::Wind, tracks)])
    }
}
