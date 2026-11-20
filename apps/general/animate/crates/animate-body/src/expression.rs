use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

use crate::names::normalized;

static ALIASES: LazyLock<HashMap<String, Expression>> = LazyLock::new(|| {
    Expression::ALL
        .into_iter()
        .flat_map(|expression| {
            expression
                .names()
                .iter()
                .map(move |name| (normalized(name), expression))
        })
        .collect()
});

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Expression {
    Happy,
    Angry,
    Sad,
    Relaxed,
    Surprised,
    Blink,
    BlinkLeft,
    BlinkRight,
    Aa,
    Ih,
    Ou,
    Ee,
    Oh,
}

pub const EXPRESSION_COUNT: usize = 13;

impl Expression {
    pub const ALL: [Self; EXPRESSION_COUNT] = [
        Self::Happy,
        Self::Angry,
        Self::Sad,
        Self::Relaxed,
        Self::Surprised,
        Self::Blink,
        Self::BlinkLeft,
        Self::BlinkRight,
        Self::Aa,
        Self::Ih,
        Self::Ou,
        Self::Ee,
        Self::Oh,
    ];

    pub const MOODS: [Self; 5] = [
        Self::Happy,
        Self::Angry,
        Self::Sad,
        Self::Relaxed,
        Self::Surprised,
    ];

    pub const BLINKS: [Self; 3] = [Self::Blink, Self::BlinkLeft, Self::BlinkRight];

    pub const VOWELS: [Self; 5] = [Self::Aa, Self::Ih, Self::Ou, Self::Ee, Self::Oh];

    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Happy => "happy",
            Self::Angry => "angry",
            Self::Sad => "sad",
            Self::Relaxed => "relaxed",
            Self::Surprised => "surprised",
            Self::Blink => "blink",
            Self::BlinkLeft => "blinkLeft",
            Self::BlinkRight => "blinkRight",
            Self::Aa => "aa",
            Self::Ih => "ih",
            Self::Ou => "ou",
            Self::Ee => "ee",
            Self::Oh => "oh",
        }
    }

    #[must_use]
    pub fn recognize(morph: &str) -> Option<Self> {
        ALIASES.get(&normalized(morph)).copied()
    }

    fn names(self) -> &'static [&'static str] {
        match self {
            Self::Happy => &["happy", "joy", "笑い", "Fcl_ALL_Joy"],
            Self::Angry => &["angry", "怒り", "Fcl_ALL_Angry"],
            Self::Sad => &["sad", "sorrow", "困る", "Fcl_ALL_Sorrow"],
            Self::Relaxed => &["relaxed", "fun", "なごみ", "Fcl_ALL_Fun"],
            Self::Surprised => &["surprised", "びっくり", "Fcl_ALL_Surprised"],
            Self::Blink => &["blink", "まばたき", "Fcl_EYE_Close", "vrc.blink"],
            Self::BlinkLeft => &[
                "blinkLeft",
                "blink_l",
                "ウィンク",
                "ウィンク２",
                "Fcl_EYE_Close_L",
                "eyeBlinkLeft",
                "vrc.blink_left",
            ],
            Self::BlinkRight => &[
                "blinkRight",
                "blink_r",
                "ウィンク右",
                "ｳｨﾝｸ２右",
                "Fcl_EYE_Close_R",
                "eyeBlinkRight",
                "vrc.blink_right",
            ],
            Self::Aa => &["aa", "a", "あ", "Fcl_MTH_A", "vrc.v_aa"],
            Self::Ih => &["ih", "i", "い", "Fcl_MTH_I", "vrc.v_ih"],
            Self::Ou => &["ou", "u", "う", "Fcl_MTH_U", "vrc.v_ou"],
            Self::Ee => &["ee", "e", "え", "Fcl_MTH_E", "vrc.v_e"],
            Self::Oh => &["oh", "o", "お", "Fcl_MTH_O", "vrc.v_oh"],
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MorphWeights {
    weights: BTreeMap<String, f32>,
}

impl MorphWeights {
    #[must_use]
    pub fn weight(&self, morph: &str) -> f32 {
        self.weights.get(morph).copied().unwrap_or(0.0)
    }

    #[must_use]
    pub fn holds(&self, morph: &str) -> bool {
        self.weights.contains_key(morph)
    }

    pub fn set(&mut self, morph: &str, weight: f32) {
        match self.weights.get_mut(morph) {
            Some(slot) => *slot = weight,
            None => {
                self.weights.insert(morph.to_owned(), weight);
            }
        }
    }

    pub fn forget(&mut self, morph: &str) {
        self.weights.remove(morph);
    }

    pub fn overlay(&mut self, over: &Self) {
        for (morph, weight) in over.iter() {
            self.set(morph, weight);
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.weights.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, f32)> {
        self.weights
            .iter()
            .map(|(morph, weight)| (morph.as_str(), *weight))
    }
}

impl FromIterator<(String, f32)> for MorphWeights {
    fn from_iter<I: IntoIterator<Item = (String, f32)>>(weights: I) -> Self {
        Self {
            weights: weights.into_iter().collect(),
        }
    }
}
