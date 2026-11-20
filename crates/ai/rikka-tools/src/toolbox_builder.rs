use std::path::PathBuf;
use std::sync::Arc;

use rikka_agent::{Function, ToolKitSet};

use crate::error::Result;
use crate::machine::Machine;
use crate::path_instructions::{Nothing, PathInstructions};
use crate::reach::Reach;
use crate::toolbox::Toolbox;

#[derive(Clone, Debug)]
pub struct ToolboxBuilder {
    pub(crate) root: PathBuf,
    pub(crate) machine: Option<Arc<dyn Machine>>,
    pub(crate) web: bool,
    pub(crate) aside: Arc<dyn PathInstructions>,
    pub(crate) tools: Vec<Function>,
    pub(crate) deferred: Vec<Function>,
    pub(crate) toolkit_sets: Vec<ToolKitSet>,
    pub(crate) reach: Reach,
}

impl ToolboxBuilder {
    #[must_use]
    pub(crate) fn new(root: impl Into<PathBuf>) -> ToolboxBuilder {
        ToolboxBuilder {
            root: root.into(),
            machine: None,
            web: false,
            aside: Arc::new(Nothing),
            tools: Vec::new(),
            deferred: Vec::new(),
            toolkit_sets: Vec::new(),
            reach: Reach::Working,
        }
    }

    #[must_use]
    pub fn machine(mut self, machine: impl Machine) -> ToolboxBuilder {
        self.machine = Some(Arc::new(machine));
        self
    }

    #[must_use]
    pub fn web(mut self) -> ToolboxBuilder {
        self.web = true;
        self
    }

    #[must_use]
    pub fn path_instructions(mut self, aside: Arc<dyn PathInstructions>) -> ToolboxBuilder {
        self.aside = aside;
        self
    }

    #[must_use]
    pub fn tools(mut self, functions: impl IntoIterator<Item = Function>) -> ToolboxBuilder {
        self.tools.extend(functions);
        self
    }

    #[must_use]
    pub fn deferred(mut self, functions: impl IntoIterator<Item = Function>) -> ToolboxBuilder {
        self.deferred.extend(functions);
        self
    }

    #[must_use]
    pub fn toolkit_set(mut self, set: ToolKitSet) -> ToolboxBuilder {
        self.toolkit_sets.push(set);
        self
    }

    #[must_use]
    pub fn read_only(mut self) -> ToolboxBuilder {
        self.reach = Reach::Reading;
        self
    }

    pub fn build(self) -> Result<Toolbox> {
        Toolbox::from_builder(self)
    }
}
