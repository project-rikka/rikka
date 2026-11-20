use std::sync::Arc;

use animate_cast::Subject;
use animate_edit::KeyId;
use animate_motion::CameraMotion;
use animate_sheet::{Lane, Sheet};
use animate_timeline::Timeline;
use animate_wind::{WindMotion, WindSetting};
use rustc_hash::FxHashSet;

fn wind() -> Arc<Sheet> {
    Arc::new(Sheet::wind(&WindMotion::default()))
}

#[test]
fn selection_lasts_while_the_subject_stays() {
    let timeline = Timeline::default();
    let speed = Lane::Wind(WindSetting::Speed);
    timeline.show(Some(Subject::Wind), wind());
    timeline.choose_lane(speed.clone());
    timeline.pick(FxHashSet::from_iter([KeyId {
        lane: speed.clone(),
        frame: 12,
    }]));
    assert_eq!(timeline.chosen_lane(), Some(speed.clone()));

    timeline.show(Some(Subject::Wind), wind());
    assert_eq!(
        timeline.chosen_lane(),
        Some(speed),
        "a fresh sheet keeps the lane"
    );
    assert!(timeline.has_picks(), "a fresh sheet keeps the picks");

    timeline.show(
        Some(Subject::Camera),
        Arc::new(Sheet::camera(&CameraMotion::default())),
    );
    assert_eq!(
        timeline.chosen_lane(),
        None,
        "another subject drops the lane"
    );
    assert!(!timeline.has_picks(), "another subject drops the picks");
}
