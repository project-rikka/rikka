use crate::cjk::CjkRegion;
use crate::face::{Face, FaceClass};
use crate::fallback::affinity::{NAMED, ScriptAffinity};
use crate::family::folds_to;
use crate::matching::StyleRequest;

const DEFAULT_HAN_REGION: CjkRegion = CjkRegion::Simplified;

pub(crate) struct Ranking<'a> {
    pub(crate) affinity: &'a ScriptAffinity,
    pub(crate) region: Option<CjkRegion>,
    pub(crate) preferred: &'static [&'static str],
    pub(crate) class: FaceClass,
    pub(crate) request: StyleRequest,
}

pub(crate) type RankKey<'f> = (usize, u8, u8, u8, u32, bool, usize, &'f str);

impl Ranking<'_> {
    pub(crate) fn key<'f>(&self, face: &'f Face) -> RankKey<'f> {
        let weight = face.record.weight;
        let desired = self.request.weight.0;
        let primary = face.folded_families[0].as_str();
        (
            self.preference(face),
            NAMED - self.affinity.score(face),
            self.region_rank(face.cjk_region),
            class_rank(face.class, self.class),
            (desired.clamp(weight.min, weight.max) - desired).abs() as u32,
            !face.record.slope.upright,
            primary.len(),
            primary,
        )
    }

    fn preference(&self, face: &Face) -> usize {
        self.preferred
            .iter()
            .position(|name| {
                face.folded_families
                    .iter()
                    .any(|family| folds_to(name, family))
            })
            .unwrap_or(self.preferred.len())
    }

    fn region_rank(&self, region: Option<CjkRegion>) -> u8 {
        match (self.region, region) {
            (Some(preferred), Some(region)) if preferred == region => 0,
            (Some(_), None) => 1,
            (Some(_), Some(_)) => 2,
            (None, None) => 0,
            (None, Some(DEFAULT_HAN_REGION)) => 1,
            (None, Some(_)) => 2,
        }
    }
}

fn class_rank(class: FaceClass, primary: FaceClass) -> u8 {
    if class == primary {
        return 0;
    }
    match class {
        FaceClass::Sans => 1,
        FaceClass::Other => 2,
        FaceClass::Serif => 3,
        FaceClass::Mono => 4,
    }
}
