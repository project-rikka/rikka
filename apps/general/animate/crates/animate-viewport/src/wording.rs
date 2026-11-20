use animate_body::Side;
use animate_solve::Handle;
use rikka_i18n::{Label, tr};

use crate::easy::Marker;

pub(crate) fn marker(marker: Marker) -> Label {
    let (side, part) = match marker {
        Marker::Eyes => return tr!("animate.marker.eyes"),
        Marker::Mouth => return tr!("animate.marker.mouth"),
        Marker::Finger(finger_marker) => (
            finger_marker.side(),
            animate_ui::finger_label(finger_marker.digit()),
        ),
        Marker::Handle(Handle::Centre) => return tr!("animate.marker.centre"),
        Marker::Handle(Handle::LowerBody) => return tr!("animate.marker.lower_body"),
        Marker::Handle(Handle::UpperBody) => return tr!("animate.marker.upper_body"),
        Marker::Handle(Handle::Head) => return tr!("animate.marker.head"),
        Marker::Handle(Handle::Shoulder(side)) => (side, tr!("animate.marker.shoulder")),
        Marker::Handle(Handle::UpperArm(side)) => (side, tr!("animate.marker.upper_arm")),
        Marker::Handle(Handle::Forearm(side)) => (side, tr!("animate.marker.elbow")),
        Marker::Handle(Handle::Hand(side)) => (side, tr!("animate.marker.hand")),
        Marker::Handle(Handle::Foot(side)) => (side, tr!("animate.marker.foot")),
        Marker::Handle(Handle::Toes(side)) => (side, tr!("animate.marker.toes")),
    };
    tr!(
        "animate.marker.sided",
        side = animate_ui::side_label(side).as_str(),
        part = part.as_str()
    )
}

pub(crate) fn grasp(side: Side) -> Label {
    tr!(
        "animate.hold.grasp",
        hand = animate_ui::holding_hand_label(side).as_str()
    )
}
