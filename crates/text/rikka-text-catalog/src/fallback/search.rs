use std::sync::Arc;

use rikka_text_unicode::{EmojiPresentation, Script};

use crate::catalog::{Catalog, FamilyId};
use crate::cjk::CjkRegion;
use crate::face::{Face, FaceId};
use crate::fallback::affinity::{ScriptAffinity, UNRELATED};
use crate::fallback::cluster::Cluster;
use crate::fallback::codepoints::CodepointIndex;
use crate::fallback::key::with_key;
use crate::fallback::rank::Ranking;
use crate::fallback::request::FallbackRequest;
use crate::family::Family;
use crate::locale::Locale;
use crate::matching::{FaceMatch, best_face};
use crate::system;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ColorFilter {
    Color,
    Monochrome,
    Any,
}

impl ColorFilter {
    fn accepts(self, face: &Face) -> bool {
        match self {
            Self::Color => face.is_color(),
            Self::Monochrome => !face.is_color(),
            Self::Any => true,
        }
    }
}

struct Search<'c, 'a> {
    catalog: &'c Catalog,
    cluster: Cluster<'a>,
    primary: FaceMatch,
    chain: Arc<[FamilyId]>,
    ranking: Ranking<'c>,
}

impl Catalog {
    pub fn fallback(&self, request: &FallbackRequest<'_>) -> FaceMatch {
        let primary = request.primary;
        let cluster = Cluster::new(request);
        let face = self.face(primary.face);
        if cluster.covered_by(face)
            && (cluster.presentation == EmojiPresentation::Text || face.is_color())
        {
            return primary;
        }
        let locale = request.locale.or(self.locale());
        with_key(&primary, &cluster, locale, |key| {
            if let Some(found) = self.fallbacks.get(key) {
                return found;
            }
            let found = self
                .search(cluster, primary, request.locale)
                .unwrap_or(primary);
            self.fallbacks.insert(key.into(), found);
            found
        })
    }

    fn search(
        &self,
        cluster: Cluster<'_>,
        primary: FaceMatch,
        locale: Option<Locale>,
    ) -> Option<FaceMatch> {
        let affinity = ScriptAffinity::new(cluster.script);
        let region = self.preferred_region(cluster.script, locale);
        let search = Search {
            catalog: self,
            cluster,
            primary,
            chain: self.chains.get(primary.chain),
            ranking: Ranking {
                affinity: &affinity,
                region,
                preferred: system::fallback_families(cluster.script, region),
                class: self.face(primary.face).class,
                request: primary.request,
            },
        };
        match cluster.presentation {
            EmojiPresentation::Emoji => search
                .in_families(self.generics.chain(&Family::Emoji), ColorFilter::Color)
                .or_else(|| search.in_any_family(ColorFilter::Color))
                .or_else(|| search.in_any_family(ColorFilter::Any)),
            EmojiPresentation::Text => search
                .in_any_family(ColorFilter::Monochrome)
                .or_else(|| search.in_any_family(ColorFilter::Any)),
        }
    }

    fn preferred_region(&self, script: Script, locale: Option<Locale>) -> Option<CjkRegion> {
        let preferred = locale
            .and_then(|locale| CjkRegion::from_locale(&locale))
            .or_else(|| {
                self.locale()
                    .and_then(|locale| CjkRegion::from_locale(&locale))
            });
        match CjkRegion::from_script(script) {
            Some(default) => Some(preferred.unwrap_or(default)),
            None if matches!(script, Script::Common | Script::Inherited) => preferred,
            None => None,
        }
    }

    pub(crate) fn script_faces(&self, script: Script, affinity: &ScriptAffinity) -> Arc<[FaceId]> {
        if let Some(faces) = self.script_faces.read().get(&script) {
            return faces.clone();
        }
        let faces: Arc<[FaceId]> = self
            .faces()
            .filter(|(_, face)| affinity.score(face) > UNRELATED)
            .map(|(id, _)| id)
            .collect();
        self.script_faces.write().insert(script, faces.clone());
        faces
    }

    fn codepoint_index(&self) -> &CodepointIndex {
        self.codepoints
            .get_or_init(|| CodepointIndex::build(&self.faces))
    }
}

impl Search<'_, '_> {
    fn accepts(&self, face: &Face, filter: ColorFilter) -> bool {
        filter.accepts(face) && self.cluster.covered_by(face)
    }

    fn in_family(&self, family: FamilyId, filter: ColorFilter) -> Option<FaceMatch> {
        let faces = &self.catalog.faces;
        let accepted = self
            .catalog
            .families
            .faces(family)
            .iter()
            .copied()
            .filter(|&id| self.accepts(&faces[id.index()], filter));
        best_face(faces, accepted, &self.primary.request, self.primary.chain)
    }

    fn in_families(&self, families: &[FamilyId], filter: ColorFilter) -> Option<FaceMatch> {
        families
            .iter()
            .find_map(|&family| self.in_family(family, filter))
    }

    fn in_any_family(&self, filter: ColorFilter) -> Option<FaceMatch> {
        self.in_families(&self.chain, filter)
            .or_else(|| self.by_script(filter))
            .or_else(|| self.by_coverage(filter))
    }

    fn by_script(&self, filter: ColorFilter) -> Option<FaceMatch> {
        let affinity = self.ranking.affinity;
        if !affinity.is_specific() {
            return None;
        }
        let faces = self.catalog.script_faces(self.cluster.script, affinity);
        self.best_ranked(&faces, filter)
    }

    fn by_coverage(&self, filter: ColorFilter) -> Option<FaceMatch> {
        let first = self.cluster.required().next()?;
        let candidates = self.catalog.codepoint_index().candidates(first);
        self.best_ranked(candidates, filter)
    }

    fn best_ranked(&self, candidates: &[FaceId], filter: ColorFilter) -> Option<FaceMatch> {
        let faces = &self.catalog.faces;
        let best = candidates
            .iter()
            .filter(|&&id| self.accepts(&faces[id.index()], filter))
            .min_by_key(|&&id| (self.ranking.key(&faces[id.index()]), id))?;
        self.catalog
            .primary_family(*best)
            .and_then(|family| self.in_family(family, filter))
    }
}
