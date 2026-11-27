mod desk;
mod drawing;
mod labels;
mod rows;

pub use desk::{Desk, Reach};
pub use drawing::{LEFT_HAND_TINT, PIN_TINT, RIGHT_HAND_TINT, centred, disc, dotted, stroke};
pub use labels::{
    anchor_label, emitter_setting_label, error_chain, finger_label, frame_clock,
    holding_hand_label, part_label, prop_setting_label, side_label, wind_setting_label,
};
pub use rows::{LABEL_WIDTH, choice, framed, glyph, heading, key_controls, labelled, toggle};

pub static CATALOGS: &[(&str, &str)] = rikka_i18n::catalogs!();
