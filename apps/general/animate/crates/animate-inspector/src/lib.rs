mod card;
mod cards;
mod controls;
mod inspector;
mod listed;
mod prop_editing;
mod studio;
mod traits;
mod wording;

pub use card::{Card, FaceTab};
pub use cards::{CORNER as CARDS_CORNER, cards};
pub use controls::{Controls, Dial, HoldDial, Pane};
pub use inspector::inspector;
pub use listed::Listed;
pub use prop_editing::PropEditing;
pub use studio::{Desk, Reach, SettingEditing, Studio, StudioCore};
pub use traits::{
    CardFocus, ClothEditing, CurveEditing, EmitterNaming, EmitterShaping, HoldEditing,
    LiquidSettings, MorphEditing, PoseEditing, PourTiming,
};

pub static CATALOGS: &[(&str, &str)] = rikka_i18n::catalogs!();
