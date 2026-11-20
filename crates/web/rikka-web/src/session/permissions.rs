use std::sync::Arc;

use rikka_gecko_protocol::Target;
use rikka_gecko_protocol::geckoview::permission::{
    ContentPermission, ContentPermissionShown, MediaPermissionDecision, MediaPermissionRequest,
    MediaSource, PermissionKind, PermissionValue,
};

use crate::wire::{Answer, Link};

#[derive(Debug)]
pub struct PermissionRequest {
    pub permission: ContentPermission,
    target: Target,
    answer: Answer,
}

impl PermissionRequest {
    pub(crate) fn new(
        permission: ContentPermission,
        target: Target,
        answer: Answer,
    ) -> PermissionRequest {
        PermissionRequest {
            permission,
            target,
            answer,
        }
    }

    #[must_use]
    pub fn kind(&self) -> Option<PermissionKind> {
        self.permission.kind()
    }

    pub fn notify_shown(&self) {
        if let Some(request_id) = self.permission.request_id.clone() {
            self.link()
                .send(self.target.clone(), &ContentPermissionShown { request_id });
        }
    }

    pub fn allow(self) {
        self.answer.with(&PermissionValue::Allow);
    }

    pub fn deny(self) {
        self.answer.with(&PermissionValue::Deny);
    }

    fn link(&self) -> &Arc<Link> {
        self.answer.link()
    }
}

#[derive(Debug)]
pub struct MediaRequest {
    pub uri: String,
    pub video: Vec<MediaSource>,
    pub audio: Vec<MediaSource>,
    answer: Answer,
}

impl MediaRequest {
    pub(crate) fn from_wire(request: MediaPermissionRequest, answer: Answer) -> MediaRequest {
        MediaRequest {
            uri: request.uri,
            video: request.video.unwrap_or_default(),
            audio: request.audio.unwrap_or_default(),
            answer,
        }
    }

    pub fn grant(self, video: Option<&MediaSource>, audio: Option<&MediaSource>) {
        self.answer.with(&MediaPermissionDecision::Grant {
            video: video.map(|source| source.id.clone()),
            audio: audio.map(|source| source.id.clone()),
        });
    }

    pub fn deny(self) {
        self.answer.with(&MediaPermissionDecision::Deny);
    }
}
