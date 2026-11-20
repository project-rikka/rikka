use jiff::Timestamp;
use std::collections::HashMap;
use std::sync::Arc;

use rikka_proton_drive::LinkId;
use rikka_types::Size;

use crate::entry::Entry;

#[derive(Clone, Debug)]
pub struct Thumb {
    pub bytes: Arc<[u8]>,
    width: u32,
    height: u32,
}

impl Thumb {
    #[must_use]
    pub fn decode(picture: Vec<u8>) -> Option<Thumb> {
        let read = rikka_image::read(picture.as_slice()).probe().ok()?;

        Some(Thumb {
            bytes: Arc::from(picture),
            width: read.width,
            height: read.height,
        })
    }

    #[must_use]
    pub fn size(&self) -> Size {
        Size::new(self.width as f32, self.height as f32)
    }
}

#[derive(Debug, Default)]
pub struct Thumbs {
    thumbs: Arc<HashMap<LinkId, Thumb>>,
    requested: HashMap<LinkId, Timestamp>,
}

impl Thumbs {
    #[must_use]
    pub fn new() -> Thumbs {
        Thumbs::default()
    }

    #[must_use]
    pub fn all(&self) -> Arc<HashMap<LinkId, Thumb>> {
        Arc::clone(&self.thumbs)
    }

    #[must_use]
    pub fn request<'a>(
        &mut self,
        entries: impl IntoIterator<Item = &'a Entry>,
    ) -> Vec<ThumbRequest> {
        let mut asking = Vec::new();
        for entry in entries {
            if !entry.pictured || self.requested.get(&entry.id) == Some(&entry.modified) {
                continue;
            }
            self.requested.insert(entry.id.clone(), entry.modified);
            asking.push(ThumbRequest {
                id: entry.id.clone(),
                at: entry.modified,
            });
        }
        asking
    }

    pub fn insert(&mut self, ask: &ThumbRequest, thumb: Option<Thumb>) -> bool {
        let (Some(thumb), true) = (thumb, self.requested.get(&ask.id) == Some(&ask.at)) else {
            return false;
        };
        Arc::make_mut(&mut self.thumbs).insert(ask.id.clone(), thumb);
        true
    }

    pub fn cancel(&mut self, ask: &ThumbRequest) {
        self.requested.remove(&ask.id);
    }

    pub fn retain(&mut self, entries: &[Entry]) {
        let listed: std::collections::HashSet<&LinkId> =
            entries.iter().map(|entry| &entry.id).collect();
        self.requested.retain(|id, _| listed.contains(id));
        if self.thumbs.keys().any(|id| !listed.contains(id)) {
            Arc::make_mut(&mut self.thumbs).retain(|id, _| listed.contains(id));
        }
    }
}

#[derive(Clone, Debug)]
pub struct ThumbRequest {
    id: LinkId,
    at: Timestamp,
}

impl ThumbRequest {
    #[must_use]
    pub fn id(&self) -> &LinkId {
        &self.id
    }
}
