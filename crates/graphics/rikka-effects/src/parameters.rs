use std::marker::PhantomData;

use rikka_image_edit::{Operation, Recipe, Step};

use crate::effect::Effect;
use crate::knob::Knob;

pub trait Knobs {
    fn knobs(&self) -> &'static [Knob];

    fn slot(&mut self, key: &str) -> Option<&mut f32>;

    fn peek(&self, key: &str) -> Option<f32>;
}

pub trait Parameters: Knobs + Clone + Default + PartialEq + Into<Effect> + 'static {
    fn operation(&self) -> Option<Operation>;

    fn normalize(&mut self, _changed: Option<&str>) {}

    fn padding(&self) -> f32 {
        0.0
    }

    fn knob(&self, key: &str) -> Option<&'static Knob> {
        self.knobs().iter().find(|knob| knob.key == key)
    }

    fn set(&mut self, key: &str, value: f32) -> bool {
        let Some(knob) = self.knob(key) else {
            return false;
        };
        if !value.is_finite() {
            return false;
        }
        let Some(slot) = self.slot(key) else {
            return false;
        };
        *slot = knob.scale.clamp(value);
        self.normalize(Some(key));
        true
    }
}

pub struct KnobKey<P> {
    name: &'static str,
    owner: PhantomData<fn() -> P>,
}

impl<P> KnobKey<P> {
    pub const fn new(name: &'static str) -> KnobKey<P> {
        KnobKey {
            name,
            owner: PhantomData,
        }
    }

    pub const fn name(self) -> &'static str {
        self.name
    }
}

impl<P> std::fmt::Debug for KnobKey<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("KnobKey").field(&self.name).finish()
    }
}

impl<P> Clone for KnobKey<P> {
    fn clone(&self) -> KnobKey<P> {
        *self
    }
}

impl<P> Copy for KnobKey<P> {}

pub(crate) fn recipe<P: Parameters>(parameters: &P) -> Recipe {
    parameters
        .operation()
        .map_or_else(Recipe::new, |operation| {
            Recipe::new().then(Step::new(operation))
        })
}

pub(crate) fn checked<P: Parameters>(mut parameters: P) -> P {
    let fresh = P::default();
    for knob in parameters.knobs() {
        let current = fresh.peek(knob.key);
        if let Some(slot) = parameters.slot(knob.key) {
            *slot = match slot.is_finite() {
                true => knob.scale.clamp(*slot),
                false => current.map_or(*knob.scale.range.start(), |value| knob.scale.clamp(value)),
            };
        }
    }
    parameters.normalize(None);
    parameters
}

macro_rules! knobs {
    (for $owner:ty;) => {
        impl $crate::parameters::Knobs for $owner {
            fn knobs(&self) -> &'static [$crate::knob::Knob] {
                &[]
            }

            fn slot(&mut self, _key: &str) -> Option<&mut f32> {
                None
            }

            fn peek(&self, _key: &str) -> Option<f32> {
                None
            }
        }
    };
    (
        for $owner:ty;
        $($constant:ident = $key:literal => $($field:ident).+ : $label:expr, $scale:expr;)+
    ) => {
        $(
            pub const $constant: $crate::parameters::KnobKey<$owner> =
                $crate::parameters::KnobKey::new($key);
        )+

        impl $crate::parameters::Knobs for $owner {
            fn knobs(&self) -> &'static [$crate::knob::Knob] {
                const KNOBS: &[$crate::knob::Knob] =
                    &[$($crate::knob::Knob::new($key, $label, $scale),)+];
                KNOBS
            }

            fn slot(&mut self, key: &str) -> Option<&mut f32> {
                match key {
                    $($key => Some(&mut self.$($field).+),)+
                    _ => None,
                }
            }

            fn peek(&self, key: &str) -> Option<f32> {
                match key {
                    $($key => Some(self.$($field).+),)+
                    _ => None,
                }
            }
        }
    };
}

macro_rules! grouped_knobs {
    (@len { $($part:ident : $name:literal, $scale:expr;)+ }) => {
        [$(stringify!($part)),+].len()
    };
    (@keys $owner:ty, $group:ident { $($part:ident : $name:literal, $scale:expr;)+ }) => {
        [$(
            $crate::parameters::KnobKey::<$owner>::new(concat!(
                stringify!($group),
                ".",
                stringify!($part)
            )),
        )+]
    };
    (@knobs $group:ident, $group_name:literal { $($part:ident : $name:literal, $scale:expr;)+ }) => {
        [$(
            $crate::knob::Knob::new(
                concat!(stringify!($group), ".", stringify!($part)),
                || $crate::knob::grouped(::rikka_i18n::tr!($group_name), ::rikka_i18n::tr!($name)),
                $scale,
            ),
        )+]
    };
    (@slot $params:ident, $part:ident, { $($each:ident : $name:literal, $scale:expr;)+ }) => {
        match $part {
            $(stringify!($each) => Some(&mut $params.$each),)+
            _ => None,
        }
    };
    (
        for $owner:ty as $keys:ident;
        $([$($group:ident : $group_name:literal),+ $(,)?] $parts:tt)+
    ) => {
        pub const $keys: [
            [$crate::parameters::KnobKey<$owner>; [$($crate::parameters::grouped_knobs!(@len $parts)),+][0]];
            [$($(stringify!($group)),+),+].len()
        ] = [$($($crate::parameters::grouped_knobs!(@keys $owner, $group $parts)),+),+];

        impl $crate::parameters::Knobs for $owner {
            fn knobs(&self) -> &'static [$crate::knob::Knob] {
                const TABLE: [
                    [$crate::knob::Knob; [$($crate::parameters::grouped_knobs!(@len $parts)),+][0]];
                    [$($(stringify!($group)),+),+].len()
                ] = [$($($crate::parameters::grouped_knobs!(@knobs $group, $group_name $parts)),+),+];
                const KNOBS: &[$crate::knob::Knob] = TABLE.as_flattened();
                KNOBS
            }

            fn slot(&mut self, key: &str) -> Option<&mut f32> {
                let (group, part) = key.split_once('.')?;
                match group {
                    $($(
                        stringify!($group) => {
                            let params = &mut self.$group;
                            $crate::parameters::grouped_knobs!(@slot params, part, $parts)
                        }
                    )+)+
                    _ => None,
                }
            }

            fn peek(&self, key: &str) -> Option<f32> {
                self.clone().slot(key).copied()
            }
        }
    };
}

pub(crate) use {grouped_knobs, knobs};
