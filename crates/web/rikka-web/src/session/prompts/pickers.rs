use std::path::PathBuf;

use rikka_gecko_protocol::geckoview::prompt::{
    AllowResponse, AuthFlags, AuthPrompt, AuthResponse, CaptureType, Choice, ChoiceMode,
    ChoicePrompt, ChoiceResponse, ColorPrompt, ColorResponse, DateTimeMode, DateTimePrompt,
    DateTimeResponse, FileMode, FilePrompt, FileResponse, FolderUploadPrompt, PromptId,
    PromptResponse,
};

use crate::error::{Error, Result};
use crate::wire::Answer;

#[derive(Debug)]
pub struct SignIn {
    pub id: PromptId,
    pub title: Option<String>,
    pub message: Option<String>,
    pub uri: Option<String>,
    pub flags: AuthFlags,
    pub username: Option<String>,
    answer: Answer,
}

impl SignIn {
    pub(crate) fn from_wire(id: PromptId, prompt: AuthPrompt, answer: Answer) -> SignIn {
        SignIn {
            id,
            title: prompt.title,
            message: prompt.msg,
            uri: prompt.options.uri,
            flags: prompt.options.flags,
            username: prompt.options.username,
            answer,
        }
    }

    pub fn submit(self, username: Option<String>, password: impl Into<String>) {
        self.answer.with(&PromptResponse::Auth(AuthResponse {
            username,
            password: password.into(),
        }));
    }

    pub fn dismiss(self) {
        drop(self);
    }
}

#[derive(Debug)]
pub struct Choose {
    pub id: PromptId,
    pub title: Option<String>,
    pub message: Option<String>,
    pub mode: ChoiceMode,
    pub choices: Vec<Choice>,
    answer: Answer,
}

impl Choose {
    pub(crate) fn from_wire(id: PromptId, prompt: ChoicePrompt, answer: Answer) -> Choose {
        Choose {
            id,
            title: prompt.title,
            message: prompt.msg,
            mode: prompt.mode,
            choices: prompt.choices,
            answer,
        }
    }

    pub fn choose<S: Into<String>>(self, ids: impl IntoIterator<Item = S>) {
        self.answer.with(&PromptResponse::Choice(ChoiceResponse {
            choices: ids.into_iter().map(Into::into).collect(),
        }));
    }

    pub fn dismiss(self) {
        drop(self);
    }
}

#[derive(Debug)]
pub struct PickColor {
    pub id: PromptId,
    pub title: Option<String>,
    pub value: Option<String>,
    pub suggested: Vec<String>,
    answer: Answer,
}

impl PickColor {
    pub(crate) fn from_wire(id: PromptId, prompt: ColorPrompt, answer: Answer) -> PickColor {
        PickColor {
            id,
            title: prompt.title,
            value: prompt.value,
            suggested: prompt.predefined_values.unwrap_or_default(),
            answer,
        }
    }

    pub fn choose(self, color: impl Into<String>) {
        self.answer.with(&PromptResponse::Color(ColorResponse {
            color: color.into(),
        }));
    }

    pub fn dismiss(self) {
        drop(self);
    }
}

#[derive(Debug)]
pub struct PickDateTime {
    pub id: PromptId,
    pub title: Option<String>,
    pub mode: DateTimeMode,
    pub value: Option<String>,
    pub min: Option<String>,
    pub max: Option<String>,
    pub step: Option<String>,
    answer: Answer,
}

impl PickDateTime {
    pub(crate) fn from_wire(id: PromptId, prompt: DateTimePrompt, answer: Answer) -> PickDateTime {
        PickDateTime {
            id,
            title: prompt.title,
            mode: prompt.mode,
            value: prompt.value,
            min: prompt.min,
            max: prompt.max,
            step: prompt.step,
            answer,
        }
    }

    pub fn choose(self, value: impl Into<String>) {
        self.answer
            .with(&PromptResponse::DateTime(DateTimeResponse {
                datetime: value.into(),
            }));
    }

    pub fn clear(self) {
        self.choose("");
    }

    pub fn dismiss(self) {
        drop(self);
    }
}

#[derive(Debug)]
pub struct PickFiles {
    pub id: PromptId,
    pub title: Option<String>,
    pub mode: FileMode,
    pub mime_types: Vec<String>,
    pub capture: CaptureType,
    answer: Answer,
}

impl PickFiles {
    pub(crate) fn from_wire(id: PromptId, prompt: FilePrompt, answer: Answer) -> PickFiles {
        PickFiles {
            id,
            title: prompt.title,
            mode: prompt.mode,
            mime_types: prompt.mime_types,
            capture: prompt.capture,
            answer,
        }
    }

    pub fn choose(self, paths: impl IntoIterator<Item = PathBuf>) -> Result<()> {
        let mut files = Vec::new();
        for path in paths {
            if !path.is_absolute() {
                return Err(Error::RelativePath(path));
            }
            files.push(path.to_string_lossy().into_owned());
        }
        self.answer.with(&PromptResponse::File(FileResponse {
            files,
            files_in_webkit_directory: None,
        }));
        Ok(())
    }

    pub fn dismiss(self) {
        drop(self);
    }
}

#[derive(Debug)]
pub struct ConfirmFolderUpload {
    pub id: PromptId,
    pub folder: String,
    answer: Answer,
}

impl ConfirmFolderUpload {
    pub(crate) fn from_wire(
        id: PromptId,
        prompt: FolderUploadPrompt,
        answer: Answer,
    ) -> ConfirmFolderUpload {
        ConfirmFolderUpload {
            id,
            folder: prompt.directory_name,
            answer,
        }
    }

    pub fn allow(self, allowed: bool) {
        self.answer
            .with(&PromptResponse::Allow(AllowResponse { allow: allowed }));
    }
}
