use std::sync::Arc;

use animate_cast::{Prop, PropRole};
use animate_prop::{Anchor, PropSetting};

pub trait PropEditing {
    fn prop_of(&self, role: PropRole) -> Option<Arc<Prop>>;
    fn prop_value(&self, role: PropRole, prop: &Prop, setting: PropSetting) -> f32;
    fn apply_prop(&mut self, role: PropRole, setting: PropSetting, value: f32);
    fn key_prop(&mut self, role: PropRole, setting: PropSetting);
    fn clear_prop(&mut self, role: PropRole, settings: &[PropSetting]);
    fn attach_prop(&mut self, role: PropRole, anchor: Option<Anchor>);
    fn key_anchor(&mut self, role: PropRole);
    fn clear_anchor(&mut self, role: PropRole);
}
