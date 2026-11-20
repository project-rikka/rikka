use rikka_sync::RwLock;
use std::collections::VecDeque;
use std::sync::{Arc, OnceLock};

use crate::common::feature::Feature;
use crate::common::segment_properties::SegmentProperties;
use crate::plan::shape_plan::PlanCore;

const SLOTS: usize = 8;
const OVERFLOW: usize = 56;

#[derive(Debug, Default)]
pub(crate) struct PlanCache {
    slots: [OnceLock<Arc<PlanCore>>; SLOTS],
    overflow: RwLock<VecDeque<Arc<PlanCore>>>,
}

impl PlanCache {
    pub(crate) fn get_or_insert(
        &self,
        properties: &SegmentProperties,
        features: &[Feature],
        build: impl FnOnce() -> Arc<PlanCore>,
    ) -> Arc<PlanCore> {
        if let Some(plan) = self.find(properties, features) {
            return plan;
        }
        let plan = build();
        for slot in &self.slots {
            let cached = slot.get_or_init(|| Arc::clone(&plan));
            if cached.matches(properties, features) {
                return Arc::clone(cached);
            }
        }
        let mut overflow = self.overflow.write();
        if let Some(cached_plan) = overflow
            .iter()
            .find(|candidate| candidate.matches(properties, features))
        {
            return Arc::clone(cached_plan);
        }
        if overflow.len() == OVERFLOW {
            overflow.pop_front();
        }
        overflow.push_back(Arc::clone(&plan));
        plan
    }

    fn find(&self, properties: &SegmentProperties, features: &[Feature]) -> Option<Arc<PlanCore>> {
        for slot in &self.slots {
            match slot.get() {
                Some(plan) if plan.matches(properties, features) => return Some(Arc::clone(plan)),
                Some(_) => {}
                None => return None,
            }
        }
        self.overflow
            .read()
            .iter()
            .find(|candidate| candidate.matches(properties, features))
            .cloned()
    }
}
