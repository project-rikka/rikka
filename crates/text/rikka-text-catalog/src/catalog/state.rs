use rikka_sync::RwLock;
use std::fmt;
use std::sync::{Arc, OnceLock};

use rikka_text_font::{Font, FontFile, Location};
use rikka_text_unicode::Script;
use rustc_hash::FxHashMap;

use crate::cache::ShardedCache;
use crate::catalog::chains::Chains;
use crate::catalog::config::CatalogConfig;
use crate::catalog::families::{FamilyId, FamilyTable};
use crate::catalog::load::{Loaded, load};
use crate::catalog::resolver::{Generics, resolve};
use crate::config::Aliases;
use crate::discovery::mapped_file;
use crate::face::{Face, FaceClass, FaceId, FaceSource};
use crate::fallback::CodepointIndex;
use crate::family::Family;
use crate::locale::Locale;
use crate::matching::{FaceMatch, best_face};
use crate::query::Query;
use crate::report::LoadReport;

const DEFAULT_SAMPLE: char = 'a';
const REGULAR_WEIGHT: f32 = 400.0;

pub struct Catalog {
    pub(crate) faces: Box<[Face]>,
    pub(crate) families: FamilyTable,
    pub(crate) generics: Generics,
    pub(crate) chains: Chains,
    pub(crate) fallbacks: ShardedCache<Box<[u8]>, FaceMatch>,
    pub(crate) codepoints: OnceLock<CodepointIndex>,
    pub(crate) script_faces: RwLock<FxHashMap<Script, Arc<[FaceId]>>>,
    aliases: Aliases,
    default_family: Option<FamilyId>,
    locale: Option<Locale>,
    report: LoadReport,
    fonts: Box<[OnceLock<Option<Font<'static>>>]>,
    queries: ShardedCache<Query, Option<FaceMatch>>,
}

impl Catalog {
    pub fn load(config: &CatalogConfig) -> Self {
        let Loaded {
            faces,
            fontconfig,
            report,
        } = load(config);
        let families = FamilyTable::build(&faces);
        let generics = Generics::build(&families, &fontconfig.aliases);
        let default_family = default_family(&faces, &families);
        Self {
            fonts: faces.iter().map(|_| OnceLock::new()).collect(),
            faces: faces.into_boxed_slice(),
            families,
            generics,
            chains: Chains::default(),
            fallbacks: ShardedCache::new(),
            codepoints: OnceLock::new(),
            script_faces: RwLock::default(),
            aliases: fontconfig.aliases,
            default_family,
            locale: config.locale,
            report,
            queries: ShardedCache::new(),
        }
    }

    #[inline]
    pub fn face(&self, id: FaceId) -> &Face {
        &self.faces[id.index()]
    }

    pub fn faces(&self) -> impl ExactSizeIterator<Item = (FaceId, &Face)> {
        self.faces
            .iter()
            .enumerate()
            .map(|(index, face)| (FaceId(index as u32), face))
    }

    pub fn families(&self) -> impl ExactSizeIterator<Item = &str> {
        self.families.names()
    }

    pub fn family_faces(&self, family: &str) -> impl Iterator<Item = FaceId> {
        self.families
            .find(family)
            .map(|id| self.families.faces(id))
            .unwrap_or_default()
            .iter()
            .copied()
    }

    pub fn font(&self, id: FaceId) -> Option<Font<'static>> {
        *self.fonts[id.index()].get_or_init(|| {
            let face = &self.faces[id.index()];
            let bytes = match &face.source {
                FaceSource::File { path, stamp } => mapped_file(path, stamp)?,
                FaceSource::Embedded(bytes) => bytes,
            };
            FontFile::new(bytes).ok()?.face(face.record.index).ok()
        })
    }

    pub fn query(&self, query: &Query) -> Option<FaceMatch> {
        if let Some(found) = self.queries.get(query) {
            return found;
        }
        let found = self.resolve_query(query);
        self.queries.insert(query.clone(), found);
        found
    }

    fn resolve_query(&self, query: &Query) -> Option<FaceMatch> {
        let resolution = resolve(
            query.families(),
            &self.families,
            &self.aliases,
            &self.generics,
        );
        let primary = resolution
            .families
            .first()
            .or_else(|| self.generics.chain(&Family::SansSerif).first())
            .copied()
            .or(self.default_family)?;
        let mut fallback = resolution.fallback;
        if !fallback.contains(&primary) {
            fallback.insert(0, primary);
        }
        let chain = self.chains.intern(fallback);
        best_face(
            &self.faces,
            self.families.faces(primary).iter().copied(),
            &query.request(),
            chain,
        )
    }

    pub fn location(&self, face_match: &FaceMatch, font_size: f32) -> Location {
        if !self.face(face_match.face).is_variable() {
            return Location::default();
        }
        self.font(face_match.face)
            .map(|font| font.location(face_match.variations(font_size)))
            .unwrap_or_default()
    }

    pub fn locale(&self) -> Option<Locale> {
        self.locale
    }

    pub fn report(&self) -> &LoadReport {
        &self.report
    }

    pub(crate) fn primary_family(&self, face: FaceId) -> Option<FamilyId> {
        self.families.primary_of(self.face(face))
    }
}

fn default_family(faces: &[Face], families: &FamilyTable) -> Option<FamilyId> {
    let face = faces.iter().min_by_key(|face| {
        let weight = face.record.weight;
        (
            !face.covers(DEFAULT_SAMPLE),
            face.class != FaceClass::Sans,
            (REGULAR_WEIGHT.clamp(weight.min, weight.max) - REGULAR_WEIGHT).abs() as u32,
            !face.record.slope.upright,
        )
    })?;
    families.primary_of(face)
}

impl fmt::Debug for Catalog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Catalog")
            .field("faces", &self.faces.len())
            .field("families", &self.families.names().len())
            .field("locale", &self.locale)
            .finish_non_exhaustive()
    }
}
