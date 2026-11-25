use slotmap::SlotMap;

use crate::desktop::Desktop;
use crate::error::{Error, Result};
use crate::ids::WindowId;
use crate::visibility::Visibility;
use crate::window::Window;
use crate::workspace::Workspace;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stack {
    Top,
    Bottom,
    Above(WindowId),
    Below(WindowId),
}

impl<P> Desktop<P> {
    pub fn raise(&mut self, id: WindowId) -> Result<bool> {
        self.restack(id, Stack::Top)
    }

    pub fn lower(&mut self, id: WindowId) -> Result<bool> {
        self.restack(id, Stack::Bottom)
    }

    pub fn restack(&mut self, id: WindowId, to: Stack) -> Result<bool> {
        let window = self.known(id)?;
        if matches!(window.visibility, Visibility::Exiting(_)) {
            return Err(Error::Exiting(id));
        }
        let unmanaged = window.unmanaged;
        if let Stack::Above(other) | Stack::Below(other) = to {
            if self.known(other)?.unmanaged != unmanaged {
                return Err(Error::SeparateStacks { window: id, other });
            }
            if other == id {
                return Ok(false);
            }
        }
        let windows = &self.windows;
        let stack = if unmanaged {
            &mut self.unmanaged_stack
        } else {
            &mut self.stack
        };
        let from = position(stack, id);
        stack.retain(|stacked| *stacked != id);
        let at = match to {
            Stack::Top => from.max(exiting_on_top(stack, windows)),
            Stack::Bottom => 0,
            Stack::Above(other) => position(stack, other) + 1,
            Stack::Below(other) => position(stack, other),
        };
        stack.insert(at, id);
        Ok(at != from)
    }

    pub fn stacking(&self) -> impl DoubleEndedIterator<Item = WindowId> + '_ {
        self.stack.iter().chain(&self.unmanaged_stack).copied()
    }

    pub fn stacked_on(
        &self,
        workspace: Workspace,
    ) -> impl DoubleEndedIterator<Item = WindowId> + '_ {
        self.stack.iter().copied().filter(move |id| {
            self.windows
                .get(*id)
                .is_some_and(|window| window.workspace == Some(workspace))
        })
    }
}

fn exiting_on_top(stack: &[WindowId], windows: &SlotMap<WindowId, Window>) -> usize {
    stack
        .iter()
        .rposition(|stacked| {
            windows
                .get(*stacked)
                .is_none_or(|window| !matches!(window.visibility, Visibility::Exiting(_)))
        })
        .map_or(0, |live| live + 1)
}

fn position(stack: &[WindowId], id: WindowId) -> usize {
    stack
        .iter()
        .position(|stacked| *stacked == id)
        .unwrap_or(stack.len())
}
