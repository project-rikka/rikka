use rikka_sync::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use rikka_text_catalog::{FaceId, FaceMatch, catalog};
use rikka_text_font::{Location, Synthesis, Tag};

use crate::fonts::face_instance::FaceInstance;
use crate::fonts::location::LocationId;

const OPTICAL_SIZE: Tag = Tag::new(b"opsz");
const ANY_SIZE: f32 = 0.0;
const MATCH_CAPACITY: usize = 4096;
const INSTANCE_CAPACITY: usize = 256;

type InstanceKey = (FaceId, LocationId, Synthesis);

static BY_MATCH: LazyLock<RwLock<HashMap<FaceMatch, Option<Arc<FaceInstance>>>>> =
    LazyLock::new(RwLock::default);
static BY_INSTANCE: LazyLock<Mutex<HashMap<InstanceKey, Arc<FaceInstance>>>> =
    LazyLock::new(Mutex::default);

pub(crate) fn face_instance(face_match: &FaceMatch) -> Option<Arc<FaceInstance>> {
    if let Some(found) = BY_MATCH.read().get(face_match) {
        return found.clone();
    }
    let created = create(face_match);
    let mut by_match = BY_MATCH.write();
    if by_match.len() >= MATCH_CAPACITY {
        by_match.clear();
    }
    by_match.entry(*face_match).or_insert(created).clone()
}

fn create(face_match: &FaceMatch) -> Option<Arc<FaceInstance>> {
    let catalog = catalog();
    let face = face_match.face();
    let font = catalog.font(face)?;
    let location = if catalog.face(face).is_variable() {
        font.location(
            face_match
                .variations(ANY_SIZE)
                .filter(|&(tag, _)| tag != OPTICAL_SIZE),
        )
    } else {
        Location::default()
    };
    let synthesis = face_match.synthesis();
    let location_id = LocationId::intern(location.clone());
    let mut instances = BY_INSTANCE.lock();
    if instances.len() >= INSTANCE_CAPACITY {
        BY_MATCH.write().clear();
        instances.retain(|_, instance| Arc::strong_count(instance) > 1);
    }
    let instance = instances
        .entry((face, location_id, synthesis))
        .or_insert_with(|| {
            Arc::new(FaceInstance::new(
                face,
                font,
                location,
                location_id,
                synthesis,
                catalog.face(face).is_color(),
            ))
        });
    Some(Arc::clone(instance))
}
