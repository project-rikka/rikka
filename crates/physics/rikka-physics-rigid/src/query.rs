use rikka_physics_collision::QueryFilter;

use crate::ShapeId;

#[must_use]
#[derive(Clone, Copy, Default)]
pub struct ShapeQuery<'a> {
    pub(crate) filter: QueryFilter,
    pub(crate) custom: Option<&'a dyn Fn(ShapeId) -> bool>,
}

impl std::fmt::Debug for ShapeQuery<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShapeQuery")
            .field("filter", &self.filter)
            .field("custom", &self.custom.is_some())
            .finish()
    }
}

impl<'a> ShapeQuery<'a> {
    pub fn new(filter: QueryFilter) -> Self {
        Self {
            filter,
            custom: None,
        }
    }

    pub fn custom(mut self, accept: &'a dyn Fn(ShapeId) -> bool) -> Self {
        self.custom = Some(accept);
        self
    }
}

impl From<QueryFilter> for ShapeQuery<'_> {
    fn from(filter: QueryFilter) -> Self {
        Self::new(filter)
    }
}
