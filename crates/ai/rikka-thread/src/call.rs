use std::sync::Arc;

use crate::attachment::Attachment;
use crate::side_effect::SideEffect;
use rikka_llm::Value;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Call {
    id: Arc<str>,
    tool: Arc<str>,
    input: Arc<Value>,
    #[serde(default)]
    output: Option<Arc<str>>,
    #[serde(default)]
    failed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    detail: Option<Arc<SideEffect>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    loaded: Vec<Arc<str>>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    hosted: bool,
    #[serde(skip)]
    pictured: Vec<Attachment>,
    #[serde(skip)]
    displayed: Vec<Attachment>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CallResult {
    pub output: Arc<str>,
    pub failed: bool,
    pub detail: Option<Arc<SideEffect>>,
    pub pictured: Vec<Attachment>,
    pub displayed: Vec<Attachment>,
    pub loaded: Vec<Arc<str>>,
}

impl Call {
    pub(crate) fn new(id: Arc<str>, tool: Arc<str>, input: Arc<Value>) -> Call {
        Call {
            id,
            tool,
            input,
            output: None,
            failed: false,
            detail: None,
            loaded: Vec::new(),
            hosted: false,
            pictured: Vec::new(),
            displayed: Vec::new(),
        }
    }

    pub(crate) fn hosted(id: Arc<str>, tool: Arc<str>, input: Arc<Value>) -> Call {
        Call {
            hosted: true,
            ..Call::new(id, tool, input)
        }
    }

    pub(crate) fn set_result(&mut self, result: CallResult) {
        self.output = Some(result.output);
        self.failed = result.failed;
        self.detail = result.detail;
        self.pictured = result.pictured;
        self.displayed = result.displayed;
        self.loaded = result.loaded;
    }

    pub(crate) fn picture(&mut self, picture: Attachment) {
        self.pictured.push(picture);
    }

    pub(crate) fn display(&mut self, picture: Attachment) {
        self.displayed.push(picture);
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn tool(&self) -> &str {
        &self.tool
    }

    #[must_use]
    pub fn input(&self) -> &Value {
        &self.input
    }

    #[must_use]
    pub fn output(&self) -> Option<&str> {
        self.output.as_deref()
    }

    #[must_use]
    pub fn detail(&self) -> Option<&SideEffect> {
        self.detail.as_deref()
    }

    #[must_use]
    pub fn displayed(&self) -> &[Attachment] {
        &self.displayed
    }

    pub(crate) fn pictured(&self) -> &[Attachment] {
        &self.pictured
    }

    pub(crate) fn loaded(&self) -> &[Arc<str>] {
        &self.loaded
    }

    #[must_use]
    pub fn is_hosted(&self) -> bool {
        self.hosted
    }

    #[must_use]
    pub fn is_failed(&self) -> bool {
        self.failed
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.output.is_none()
    }
}
