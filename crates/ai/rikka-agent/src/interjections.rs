use rikka_sync::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;

use rikka_llm::Content;

#[derive(Clone, Debug, PartialEq)]
pub struct Interjection {
    id: u64,
    content: Vec<Content>,
}

impl Interjection {
    #[must_use]
    pub fn new(id: u64, content: Vec<Content>) -> Interjection {
        Interjection { id, content }
    }

    #[must_use]
    pub fn id(&self) -> u64 {
        self.id
    }

    #[must_use]
    pub fn content(&self) -> &[Content] {
        &self.content
    }
}

#[derive(Clone, Debug, Default)]
pub struct Interjections {
    posted: Arc<Mutex<VecDeque<Interjection>>>,
}

impl Interjections {
    #[must_use]
    pub fn new() -> Interjections {
        Interjections::default()
    }

    pub fn post(&self, interjection: Interjection) {
        self.posted.lock().push_back(interjection);
    }

    pub fn withdraw(&self, id: u64) -> bool {
        let mut posted = self.posted.lock();
        let before = posted.len();
        posted.retain(|interjection| interjection.id != id);
        posted.len() != before
    }

    #[must_use]
    pub fn take(&self) -> Vec<Interjection> {
        self.posted.lock().drain(..).collect()
    }
}
