use super::spring::Spring;

#[must_use]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drive {
    pub(crate) spring: Option<Spring>,
    pub(crate) max_effort: f32,
}

impl Drive {
    pub fn spring(spring: Spring) -> Self {
        Self {
            spring: Some(spring),
            max_effort: f32::MAX,
        }
    }

    pub fn velocity() -> Self {
        Self {
            spring: None,
            max_effort: f32::MAX,
        }
    }

    pub fn max_effort(mut self, effort: f32) -> Self {
        self.max_effort = effort;
        self
    }

    pub fn spring_setting(&self) -> Option<Spring> {
        self.spring
    }

    pub fn effort_limit(&self) -> f32 {
        self.max_effort
    }

    pub(crate) fn is_valid(&self) -> bool {
        self.max_effort >= 0.0 && self.spring.is_none_or(|spring| spring.is_valid())
    }
}
