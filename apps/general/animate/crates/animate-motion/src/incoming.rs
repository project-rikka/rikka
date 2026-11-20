use crate::take::Take;

pub trait Merge: Clone {
    fn merge_into(&mut self, other: Self);
}

impl Merge for Take {
    fn merge_into(&mut self, other: Self) {
        self.merge(other);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Incoming<M> {
    motion: M,
    replaces: bool,
}

impl<M: Merge> Incoming<M> {
    #[must_use]
    pub fn merging(motion: M) -> Self {
        Self {
            motion,
            replaces: false,
        }
    }

    #[must_use]
    pub fn replacing(motion: M) -> Self {
        Self {
            motion,
            replaces: true,
        }
    }

    pub fn then(&mut self, next: Self) {
        match next.replaces {
            true => *self = next,
            false => self.motion.merge_into(next.motion),
        }
    }

    #[must_use]
    pub fn onto(&self, standing: &M) -> M {
        if self.replaces {
            return self.motion.clone();
        }
        let mut landed = standing.clone();
        landed.merge_into(self.motion.clone());
        landed
    }
}
