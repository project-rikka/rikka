use std::sync::{Arc, Mutex};

use crate::request::Request;

#[derive(Debug, Clone, Default)]
pub struct Record(Arc<Mutex<Vec<Request>>>);

impl Record {
    pub fn push(&self, request: &Request) {
        self.0.lock().unwrap().push(request.clone());
    }

    #[must_use]
    pub fn requests(&self) -> Vec<Request> {
        self.0.lock().unwrap().clone()
    }

    #[must_use]
    pub fn matching(&self, method: &str, path: &str) -> Vec<Request> {
        self.requests()
            .into_iter()
            .filter(|request| request.route() == (method, path))
            .collect()
    }

    #[must_use]
    pub fn count(&self, method: &str, path: &str) -> usize {
        self.matching(method, path).len()
    }

    pub fn last(&self, method: &str, path: &str) -> Request {
        self.matching(method, path)
            .pop()
            .unwrap_or_else(|| panic!("no request recorded for {method} {path}"))
    }
}
