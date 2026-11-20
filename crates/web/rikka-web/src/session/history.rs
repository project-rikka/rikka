use rikka_gecko_protocol::geckoview::history::{
    GetHostVisitedSince, GetVisited, OnVisited, VisitFlags,
};

use crate::wire::Answer;

#[derive(Debug)]
pub struct Visit {
    pub uri: String,
    pub last_visited: Option<String>,
    pub flags: VisitFlags,
    answer: Answer,
}

impl Visit {
    pub(crate) fn from_wire(visit: OnVisited, answer: Answer) -> Visit {
        Visit {
            uri: visit.url,
            last_visited: visit.last_visited_url,
            flags: visit.flags,
            answer,
        }
    }

    pub fn answer(self, recorded: bool) {
        self.answer.with(&recorded);
    }
}

#[derive(Debug)]
pub struct VisitedQuery {
    pub uris: Vec<String>,
    answer: Answer,
}

impl VisitedQuery {
    pub(crate) fn from_wire(query: GetVisited, answer: Answer) -> VisitedQuery {
        VisitedQuery {
            uris: query.urls,
            answer,
        }
    }

    pub fn answer(self, visited: Vec<bool>) {
        self.answer.with(&Some(visited));
    }
}

#[derive(Debug)]
pub struct HostVisitedQuery {
    pub host: String,
    pub after: i64,
    pub before: i64,
    answer: Answer,
}

impl HostVisitedQuery {
    pub(crate) fn from_wire(query: GetHostVisitedSince, answer: Answer) -> HostVisitedQuery {
        HostVisitedQuery {
            host: query.host,
            after: query.after,
            before: query.before,
            answer,
        }
    }

    pub fn answer(self, visited: bool) {
        self.answer.with(&Some(visited));
    }
}
