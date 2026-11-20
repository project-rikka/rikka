use std::any::Any;
use std::process::Command;
use std::sync::Arc;

use rikka_computer::{Computer, Kind};
use rikka_prompt::{Facts, Prompt};
use rikka_seam::SeamClient;
use rikka_tool::Function;
use rikka_tool_docs::{COMPUTER, DESKTOP};
use rikka_tools::{Machine, Result};

use crate::tool;

#[derive(Clone, Debug)]
pub struct ComputerUse {
    computer: Computer,
}

impl ComputerUse {
    #[must_use]
    pub fn machine() -> ComputerUse {
        ComputerUse::from(Computer::machine())
    }

    #[must_use]
    pub fn desktop(seam: SeamClient) -> ComputerUse {
        ComputerUse::from(Computer::desktop(seam))
    }

    #[must_use]
    pub fn computer(&self) -> &Computer {
        &self.computer
    }

    #[must_use]
    pub fn computer_of(machine: &dyn Machine) -> Option<&Computer> {
        let machine: &dyn Any = machine;
        machine
            .downcast_ref::<ComputerUse>()
            .map(ComputerUse::computer)
    }
}

impl From<Computer> for ComputerUse {
    fn from(computer: Computer) -> ComputerUse {
        ComputerUse { computer }
    }
}

impl Machine for ComputerUse {
    fn document(&self) -> &'static str {
        match self.computer.kind() {
            Kind::Machine => COMPUTER,
            Kind::Desktop => DESKTOP,
        }
    }

    fn tool(&self, prompt: &Prompt, facts: &Facts) -> Result<Function> {
        tool::declare(&self.computer, prompt, facts)
    }

    fn isolated(&self) -> Arc<dyn Machine> {
        Arc::new(ComputerUse::from(self.computer.isolated()))
    }

    fn enter(&self, command: &mut Command) {
        if let Some(environment) = self.computer.environment() {
            environment.apply(command);
        }
    }
}
