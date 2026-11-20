use std::sync::Arc;

use rikka_ui::{Label, MaterialSymbol};

use crate::provider::{Activation, ChangeStream, Hit, Layout};

pub(crate) trait Command: std::fmt::Debug + 'static {
    fn info(&self) -> CommandInfo;

    fn open(&mut self) -> Vec<Hit>;

    fn reopened(&mut self) -> Option<Vec<Hit>> {
        None
    }

    fn apply(&mut self, id: &str) -> Activation;

    fn changes(&self) -> Option<ChangeStream> {
        None
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CommandInfo {
    id: Arc<str>,
    invocation: Arc<str>,
    label: Label,
    summary: Option<Label>,
    icon: MaterialSymbol,
    layout: Layout,
}

impl CommandInfo {
    #[must_use]
    pub(crate) fn new(id: impl Into<Arc<str>>, label: impl Into<Label>) -> CommandInfo {
        let id: Arc<str> = id.into();
        CommandInfo {
            invocation: format!("{id} ").into(),
            id,
            label: label.into(),
            summary: None,
            icon: MaterialSymbol::BOLT,
            layout: Layout::List,
        }
    }

    #[must_use]
    pub(crate) fn summary(mut self, summary: impl Into<Label>) -> CommandInfo {
        self.summary = Some(summary.into());
        self
    }

    #[must_use]
    pub(crate) fn icon(mut self, icon: MaterialSymbol) -> CommandInfo {
        self.icon = icon;
        self
    }

    #[must_use]
    pub(crate) fn gallery(mut self) -> CommandInfo {
        self.layout = Layout::Gallery;
        self
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(super) fn is_named(&self, name: &str) -> bool {
        self.id.eq_ignore_ascii_case(name)
    }

    pub(crate) fn layout(&self) -> Layout {
        self.layout
    }

    pub(super) fn invocation(&self) -> Arc<str> {
        self.invocation.clone()
    }

    pub(crate) fn row(&self) -> Hit {
        let hit = Hit::new(self.id(), self.label.retranslated().as_str())
            .icon(self.icon)
            .completes(&*self.invocation);
        match &self.summary {
            Some(summary) => hit.subtitle(summary.clone()),
            None => hit,
        }
    }
}
