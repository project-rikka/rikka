use std::fmt;

use rikka_compositor_desktop::Workspace;
use rikka_compositor_protocols::ext_workspace_group_handle_v1::{
    ExtWorkspaceGroupHandleV1, GroupCapabilities,
};
use rikka_compositor_protocols::ext_workspace_handle_v1::{
    ExtWorkspaceHandleV1, State, WorkspaceCapabilities,
};
use rikka_compositor_protocols::ext_workspace_manager_v1::ExtWorkspaceManagerV1;
use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_server::Client;
use rikka_compositor_types::OutputId;
use rikka_session::workspaces::COUNT;

use rikka_compositor_output::Outputs as WaylandOutputs;

#[derive(Default)]
pub(crate) struct WorkspaceList {
    shown: Vec<Workspace>,
    managers: Vec<Manager>,
}

struct Manager {
    manager: ExtWorkspaceManagerV1,
    groups: Vec<ExtWorkspaceGroupHandleV1>,
    workspaces: Vec<ExtWorkspaceHandleV1>,
    activating: Vec<Workspace>,
}

impl WorkspaceList {
    pub(crate) fn publish(&mut self, shown: Vec<Workspace>, outputs: &WaylandOutputs) {
        if shown == self.shown {
            return;
        }
        let before = std::mem::replace(&mut self.shown, shown);
        for manager in &mut self.managers {
            manager.follow(&before, &self.shown, outputs);
        }
    }

    pub(super) fn bound(&mut self, manager: ExtWorkspaceManagerV1, outputs: &WaylandOutputs) {
        let mut entry = Manager {
            manager,
            groups: Vec::new(),
            workspaces: Vec::new(),
            activating: Vec::new(),
        };
        for shown in &self.shown {
            entry.introduce(*shown, outputs);
        }
        entry.manager.done();
        self.managers.push(entry);
    }

    pub(crate) fn output_bound(&mut self, output: OutputId, resource: &WlOutput) {
        for manager in &mut self.managers {
            if manager.manager.client() != resource.client() {
                continue;
            }
            let groups: Vec<&ExtWorkspaceGroupHandleV1> = manager
                .groups
                .iter()
                .filter(|group| group.alive() && group.key::<OutputId>() == Some(output))
                .collect();
            if groups.is_empty() {
                continue;
            }
            for group in groups {
                group.output_enter(resource);
            }
            manager.manager.done();
        }
    }

    pub(super) fn activate(&mut self, workspace: &ExtWorkspaceHandleV1) {
        let Some(target) = listed(workspace) else {
            return;
        };
        if let Some(manager) = self
            .managers
            .iter_mut()
            .find(|manager| manager.workspaces.contains(workspace))
        {
            manager
                .activating
                .retain(|pending| pending.output() != target.output());
            manager.activating.push(target);
        }
    }

    pub(super) fn commit(&mut self, manager: &ExtWorkspaceManagerV1) -> Vec<Workspace> {
        self.managers
            .iter_mut()
            .find(|entry| entry.manager == *manager)
            .map(|entry| std::mem::take(&mut entry.activating))
            .unwrap_or_default()
    }

    pub(super) fn stop(&mut self, manager: &ExtWorkspaceManagerV1) {
        self.forget(manager);
        manager.finished();
    }

    pub(super) fn forget(&mut self, manager: &ExtWorkspaceManagerV1) {
        self.managers.retain(|entry| entry.manager != *manager);
    }

    pub(super) fn forget_group(&mut self, group: &ExtWorkspaceGroupHandleV1) {
        for manager in &mut self.managers {
            manager.groups.retain(|other| other != group);
        }
    }

    pub(super) fn forget_workspace(&mut self, workspace: &ExtWorkspaceHandleV1) {
        for manager in &mut self.managers {
            manager.workspaces.retain(|other| other != workspace);
        }
    }
}

impl Manager {
    fn follow(&mut self, before: &[Workspace], after: &[Workspace], outputs: &WaylandOutputs) {
        let mut changed = false;
        for gone in before
            .iter()
            .filter(|old| !after.iter().any(|new| new.output() == old.output()))
        {
            self.retire(gone.output());
            changed = true;
        }
        for now in after {
            match before.iter().find(|old| old.output() == now.output()) {
                None => self.introduce(*now, outputs),
                Some(old) if old != now => {
                    self.show(*old, false);
                    self.show(*now, true);
                }
                Some(_) => continue,
            }
            changed = true;
        }
        if changed {
            self.manager.done();
        }
    }

    fn introduce(&mut self, shown: Workspace, outputs: &WaylandOutputs) {
        let Some(client) = Client::from_resource(&self.manager) else {
            return;
        };
        let output = shown.output();
        let version = self.manager.version();
        let group = client.create::<ExtWorkspaceGroupHandleV1>(version);
        group.set_key(output);
        self.manager.workspace_group(&group);
        group.capabilities(GroupCapabilities::empty());
        for resource in outputs.resources(output, client.id()) {
            group.output_enter(&resource);
        }
        for number in 1..=COUNT {
            let workspace = client.create::<ExtWorkspaceHandleV1>(version);
            let listed = Workspace::new(output, number);
            list(&workspace, listed);
            self.manager.workspace(&workspace);
            workspace.name(&number.to_string());
            workspace.coordinates(&coordinates(number));
            workspace.state(state(listed == shown));
            workspace.capabilities(WorkspaceCapabilities::ACTIVATE);
            group.workspace_enter(&workspace);
            self.workspaces.push(workspace);
        }
        self.groups.push(group);
    }

    fn retire(&mut self, output: OutputId) {
        let (groups, remaining): (Vec<_>, Vec<_>) = std::mem::take(&mut self.groups)
            .into_iter()
            .partition(|group| !group.alive() || group.key::<OutputId>() == Some(output));
        self.groups = remaining;
        let (workspaces, remaining): (Vec<_>, Vec<_>) = std::mem::take(&mut self.workspaces)
            .into_iter()
            .partition(|workspace| {
                !workspace.alive() || workspace.key::<OutputId>() == Some(output)
            });
        self.workspaces = remaining;
        self.activating.retain(|pending| pending.output() != output);
        for group in groups.iter().filter(|group| group.alive()) {
            for workspace in workspaces.iter().filter(|workspace| workspace.alive()) {
                group.workspace_leave(workspace);
            }
        }
        for workspace in &workspaces {
            workspace.removed();
        }
        for group in &groups {
            group.removed();
        }
    }

    fn show(&self, target: Workspace, active: bool) {
        for workspace in self
            .workspaces
            .iter()
            .filter(|workspace| listed(workspace) == Some(target))
        {
            workspace.state(state(active));
        }
    }
}

fn state(active: bool) -> State {
    match active {
        true => State::ACTIVE,
        false => State::empty(),
    }
}

fn coordinates(number: usize) -> [u8; 4] {
    u32::try_from(number - 1).unwrap_or(u32::MAX).to_ne_bytes()
}

impl fmt::Debug for WorkspaceList {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkspaceList")
            .field("shown", &self.shown)
            .field("managers", &self.managers.len())
            .finish()
    }
}

#[derive(Debug, Clone, Copy)]
struct WorkspaceNumber(usize);

impl rikka_compositor_server::Key for WorkspaceNumber {
    fn into_raw(self) -> u64 {
        self.0 as u64
    }

    fn from_raw(raw: u64) -> Self {
        Self(raw as usize)
    }
}

fn list(workspace: &ExtWorkspaceHandleV1, listed: Workspace) {
    workspace.set_key(listed.output());
    workspace.set_key(WorkspaceNumber(listed.number()));
}

fn listed(workspace: &ExtWorkspaceHandleV1) -> Option<Workspace> {
    Some(Workspace::new(
        workspace.key::<OutputId>()?,
        workspace.key::<WorkspaceNumber>()?.0,
    ))
}
