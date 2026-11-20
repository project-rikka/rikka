use rikka_gecko_protocol::geckoview::prompt::{
    AllowResponse, PromptId, PromptResponse, SharePrompt, ShareResponse, ShareResult, TargetPrompt,
    TargetResponse,
};

use crate::wire::Answer;

#[derive(Debug)]
pub struct AllowPopup {
    pub id: PromptId,
    pub target: Option<String>,
    answer: Answer,
}

impl AllowPopup {
    pub(crate) fn from_wire(id: PromptId, prompt: TargetPrompt, answer: Answer) -> AllowPopup {
        AllowPopup {
            id,
            target: prompt.target_uri,
            answer,
        }
    }

    pub fn allow(self, allowed: bool) {
        self.answer.with(&PromptResponse::Target(TargetResponse {
            response: allowed,
        }));
    }
}

#[derive(Debug)]
pub struct AllowRedirect {
    pub id: PromptId,
    pub target: Option<String>,
    answer: Answer,
}

impl AllowRedirect {
    pub(crate) fn from_wire(id: PromptId, prompt: TargetPrompt, answer: Answer) -> AllowRedirect {
        AllowRedirect {
            id,
            target: prompt.target_uri,
            answer,
        }
    }

    pub fn allow(self, allowed: bool) {
        self.answer.with(&PromptResponse::Target(TargetResponse {
            response: allowed,
        }));
    }
}

#[derive(Debug)]
pub struct ConfirmRepost {
    pub id: PromptId,
    answer: Answer,
}

impl ConfirmRepost {
    pub(crate) fn from_wire(id: PromptId, answer: Answer) -> ConfirmRepost {
        ConfirmRepost { id, answer }
    }

    pub fn allow(self, allowed: bool) {
        self.answer
            .with(&PromptResponse::Allow(AllowResponse { allow: allowed }));
    }
}

#[derive(Debug)]
pub struct ConfirmLeave {
    pub id: PromptId,
    answer: Answer,
}

impl ConfirmLeave {
    pub(crate) fn from_wire(id: PromptId, answer: Answer) -> ConfirmLeave {
        ConfirmLeave { id, answer }
    }

    pub fn leave(self, leaving: bool) {
        self.answer
            .with(&PromptResponse::Allow(AllowResponse { allow: leaving }));
    }
}

#[derive(Debug)]
pub struct Share {
    pub id: PromptId,
    pub title: Option<String>,
    pub text: Option<String>,
    pub uri: Option<String>,
    answer: Answer,
}

impl Share {
    pub(crate) fn from_wire(id: PromptId, prompt: SharePrompt, answer: Answer) -> Share {
        Share {
            id,
            title: prompt.title,
            text: prompt.text,
            uri: prompt.uri,
            answer,
        }
    }

    pub fn finish(self, result: ShareResult) {
        self.answer
            .with(&PromptResponse::Share(ShareResponse { response: result }));
    }
}
