#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Filter {
    pub category: u64,
    pub mask: u64,
    pub group: i32,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            category: 1,
            mask: u64::MAX,
            group: 0,
        }
    }
}

impl Filter {
    #[must_use]
    pub fn category(mut self, category: u64) -> Self {
        self.category = category;
        self
    }

    #[must_use]
    pub fn mask(mut self, mask: u64) -> Self {
        self.mask = mask;
        self
    }

    #[must_use]
    pub fn group(mut self, group: i32) -> Self {
        self.group = group;
        self
    }

    #[inline]
    pub fn should_collide(&self, other: &Self) -> bool {
        if self.group == other.group && self.group != 0 {
            return self.group > 0;
        }
        self.mask & other.category != 0 && self.category & other.mask != 0
    }

    #[inline]
    pub fn accepts(&self, query: &QueryFilter) -> bool {
        self.category & query.mask != 0 && query.category & self.mask != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct QueryFilter {
    pub category: u64,
    pub mask: u64,
}

impl Default for QueryFilter {
    fn default() -> Self {
        Self {
            category: u64::MAX,
            mask: u64::MAX,
        }
    }
}

impl QueryFilter {
    #[must_use]
    pub fn category(mut self, category: u64) -> Self {
        self.category = category;
        self
    }

    #[must_use]
    pub fn mask(mut self, mask: u64) -> Self {
        self.mask = mask;
        self
    }
}

pub trait PairFilter: Sync {
    fn should_collide(&self, collider_a: u32, collider_b: u32) -> bool;
}

impl<F: Fn(u32, u32) -> bool + Sync> PairFilter for F {
    fn should_collide(&self, collider_a: u32, collider_b: u32) -> bool {
        self(collider_a, collider_b)
    }
}
