use rikka_compositor_types::OutputId;

use crate::desktop::Desktop;
use crate::error::{Error, Result};
use crate::ids::WindowId;
use crate::visibility::Visibility;
use crate::workspace::Workspace;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Switch {
    pub from: Workspace,
    pub leaving: Vec<WindowId>,
    pub arriving: Vec<WindowId>,
}

impl<P> Desktop<P> {
    pub fn showing(&self, output: OutputId) -> Option<Workspace> {
        self.output_record(output)
            .map(|record| Workspace::new(output, record.showing))
    }

    pub fn show_workspace(&mut self, workspace: Workspace) -> Result<Option<Switch>> {
        let workspace = self.checked(workspace)?;
        let output = workspace.output();
        let from = self.showing(output).ok_or(Error::UnknownOutput(output))?;
        if from == workspace {
            return Ok(None);
        }
        let leaving = self.shown_on(from);
        if let Some(record) = self.output_state_mut(output) {
            record.showing = workspace.number();
        }
        let touched: Vec<WindowId> = self
            .stack
            .iter()
            .copied()
            .filter(|id| {
                self.windows.get(*id).is_some_and(|window| {
                    window.workspace == Some(from) || window.workspace == Some(workspace)
                })
            })
            .collect();
        for id in touched {
            self.settle(id);
        }
        Ok(Some(Switch {
            from,
            leaving,
            arriving: self.shown_on(workspace),
        }))
    }

    pub fn move_to_workspace(&mut self, id: WindowId, workspace: Workspace) -> Result<()> {
        let workspace = self.checked(workspace)?;
        let window = self.known_mut(id)?;
        if window.unmanaged {
            return Err(Error::Unmanaged(id));
        }
        if matches!(window.visibility, Visibility::Exiting(_)) {
            return Err(Error::Exiting(id));
        }
        window.workspace = Some(workspace);
        self.settle(id);
        self.raise(id)?;
        Ok(())
    }

    fn shown_on(&self, workspace: Workspace) -> Vec<WindowId> {
        self.stacked_on(workspace)
            .filter(|id| {
                self.windows
                    .get(*id)
                    .is_some_and(|window| window.visibility.shown())
            })
            .collect()
    }
}
