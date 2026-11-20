use std::cell::Cell;
use std::rc::Rc;

use rikka_ui::{Observed, Observers};

#[derive(Clone, Debug)]
pub struct Ratio(Rc<SharedRatio>);

#[derive(Debug)]
struct SharedRatio {
    value: Cell<f32>,
    observers: Observers,
}

impl Ratio {
    #[must_use]
    pub fn new(value: f32) -> Ratio {
        Ratio(Rc::new(SharedRatio {
            value: Cell::new(value),
            observers: Observers::new(),
        }))
    }

    pub fn get(&self) -> f32 {
        self.0.observers.read();
        self.0.value.get()
    }

    pub fn set(&self, value: f32) {
        if self.0.value.replace(value) != value {
            self.0.observers.reshaped();
        }
    }

    #[must_use]
    pub fn first(&self) -> SplitRatios {
        SplitRatios::part(self, true)
    }

    #[must_use]
    pub fn second(&self) -> SplitRatios {
        SplitRatios::part(self, false)
    }

    fn current(&self) -> f32 {
        self.0.value.get()
    }
}

impl PartialEq for Ratio {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Observed for Ratio {
    fn observers(&self) -> &Observers {
        &self.0.observers
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SplitRatios {
    base: f32,
    parts: Vec<(Ratio, bool)>,
}

impl SplitRatios {
    fn part(ratio: &Ratio, first: bool) -> SplitRatios {
        SplitRatios {
            base: 1.0,
            parts: vec![(ratio.clone(), first)],
        }
    }

    #[must_use]
    pub fn nested_in(mut self, outer: SplitRatios) -> SplitRatios {
        self.base *= outer.base;
        self.parts.extend(outer.parts);
        self
    }

    pub fn get(&self) -> f32 {
        for (ratio, _) in &self.parts {
            ratio.0.observers.read();
        }
        self.value()
    }

    pub(crate) fn value(&self) -> f32 {
        self.parts
            .iter()
            .fold(self.base, |share, (ratio, first)| match first {
                true => share * ratio.current(),
                false => share * (1.0 - ratio.current()),
            })
    }

    pub(crate) fn watch(&self) {
        for (ratio, _) in &self.parts {
            ratio.0.observers.painting();
        }
    }
}

impl From<f32> for SplitRatios {
    fn from(share: f32) -> Self {
        SplitRatios {
            base: share,
            parts: Vec::new(),
        }
    }
}

impl From<&Ratio> for SplitRatios {
    fn from(ratio: &Ratio) -> Self {
        ratio.first()
    }
}
