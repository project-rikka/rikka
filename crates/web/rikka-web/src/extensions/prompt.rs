use rikka_gecko_protocol::geckoview::webextension::extension::{Extension, PermissionSet};
use rikka_gecko_protocol::geckoview::webextension::prompt::{
    AllowResponse, InstallPrompt, InstallPromptResponse, OptionalPrompt, UpdatePrompt,
};

use crate::wire::Answer;

#[derive(Debug)]
pub struct InstallRequest {
    pub extension: Extension,
    pub permissions: PermissionSet,
    answer: Answer,
}

impl InstallRequest {
    pub(crate) fn from_wire(prompt: InstallPrompt, answer: Answer) -> InstallRequest {
        InstallRequest {
            extension: prompt.extension,
            permissions: PermissionSet {
                permissions: prompt.permissions,
                origins: prompt.origins,
                data_collection: prompt.data_collection_permissions,
            },
            answer,
        }
    }

    pub fn allow(self, in_private: bool, technical_data: bool) {
        self.answer.with(&InstallPromptResponse {
            allow: true,
            private_browsing_allowed: in_private,
            is_technical_and_interaction_data_granted: technical_data,
        });
    }

    pub fn deny(self) {
        drop(self);
    }
}

#[derive(Debug)]
pub struct UpdateRequest {
    pub extension: Extension,
    pub added: PermissionSet,
    answer: Answer,
}

impl UpdateRequest {
    pub(crate) fn from_wire(prompt: UpdatePrompt, answer: Answer) -> UpdateRequest {
        UpdateRequest {
            extension: prompt.extension,
            added: PermissionSet {
                permissions: prompt.new_permissions,
                origins: prompt.new_origins,
                data_collection: prompt.new_data_collection_permissions,
            },
            answer,
        }
    }

    pub fn allow(self, allowed: bool) {
        self.answer.with(&AllowResponse { allow: allowed });
    }
}

#[derive(Debug)]
pub struct OptionalPermissionsRequest {
    pub extension_id: String,
    pub permissions: PermissionSet,
    answer: Answer,
}

impl OptionalPermissionsRequest {
    pub(crate) fn from_wire(prompt: OptionalPrompt, answer: Answer) -> OptionalPermissionsRequest {
        OptionalPermissionsRequest {
            extension_id: prompt.extension_id,
            permissions: prompt.permissions,
            answer,
        }
    }

    pub fn allow(self, allowed: bool) {
        self.answer.with(&AllowResponse { allow: allowed });
    }
}
