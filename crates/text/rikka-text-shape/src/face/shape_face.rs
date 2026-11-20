use std::sync::Arc;

use rikka_text_font::{Font, Location};

use crate::common::feature::Feature;
use crate::common::segment_properties::SegmentProperties;
use crate::face::font_context::FontContext;
use crate::plan::plan_cache::PlanCache;
use crate::plan::planner;
use crate::plan::shape_plan::ShapePlan;

#[derive(Debug)]
pub struct ShapeFace {
    pub(crate) context: FontContext,
    plans: PlanCache,
}

impl ShapeFace {
    pub fn new(font: Font<'static>, location: Location) -> Arc<Self> {
        Arc::new(Self {
            context: FontContext::new(font, location),
            plans: PlanCache::default(),
        })
    }

    pub fn font(&self) -> Font<'static> {
        self.context.font
    }

    pub fn location(&self) -> &Location {
        &self.context.location
    }

    pub fn units_per_em(&self) -> u16 {
        self.context.font.units_per_em()
    }

    pub fn plan(
        self: &Arc<Self>,
        properties: &SegmentProperties,
        features: &[Feature],
    ) -> ShapePlan {
        let core = self.plans.get_or_insert(properties, features, || {
            planner::compile(&self.context, properties, features)
        });
        ShapePlan::new(Arc::clone(self), core)
    }
}
