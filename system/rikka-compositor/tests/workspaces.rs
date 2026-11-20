use rikka_compositor::Extent;
use rikka_seam::Request;
use rikka_session::workspaces::{CAPABILITY, SHOW, STATE, State};
use rikka_test_display::Client;
use rikka_wayland_client::ext_workspace_group_handle_v1::{self, ExtWorkspaceGroupHandleV1};
use rikka_wayland_client::ext_workspace_handle_v1::{
    self, ExtWorkspaceHandleV1, WorkspaceCapabilities,
};
use rikka_wayland_client::ext_workspace_manager_v1::{self, ExtWorkspaceManagerV1};
use rikka_wayland_client::wl_output::{self, WlOutput};

use crate::common::{Harness, PATIENCE};

const EXTENT: Extent = Extent {
    width: 320,
    height: 240,
};

#[derive(Debug, Default)]
struct Listed {
    groups: Vec<Group>,
    workspaces: Vec<Workspace>,
    done: usize,
}

#[derive(Debug)]
struct Group {
    handle: ExtWorkspaceGroupHandleV1,
    outputs: Vec<WlOutput>,
    members: Vec<ExtWorkspaceHandleV1>,
    removed: bool,
}

#[derive(Debug)]
struct Workspace {
    handle: ExtWorkspaceHandleV1,
    name: String,
    coordinates: Vec<u32>,
    active: bool,
    capabilities: WorkspaceCapabilities,
    removed: bool,
}

impl Listed {
    fn workspace(&mut self, handle: &ExtWorkspaceHandleV1) -> Option<&mut Workspace> {
        self.workspaces
            .iter_mut()
            .find(|workspace| workspace.handle == *handle)
    }

    fn group(&mut self, handle: &ExtWorkspaceGroupHandleV1) -> Option<&mut Group> {
        self.groups.iter_mut().find(|group| group.handle == *handle)
    }

    fn active(&self) -> Vec<&str> {
        self.workspaces
            .iter()
            .filter(|workspace| workspace.active && !workspace.removed)
            .map(|workspace| workspace.name.as_str())
            .collect()
    }

    fn named(&self, name: &str) -> &ExtWorkspaceHandleV1 {
        &self
            .workspaces
            .iter()
            .find(|workspace| workspace.name == name)
            .expect("workspace listed")
            .handle
    }
}

fn on_wl_output(_: &mut Listed, _: WlOutput, _: wl_output::Event) {}

fn on_ext_workspace_manager_v1(
    listed: &mut Listed,
    _: ExtWorkspaceManagerV1,
    event: ext_workspace_manager_v1::Event,
) {
    match event {
        ext_workspace_manager_v1::Event::WorkspaceGroup { workspace_group } => {
            listed.groups.push(Group {
                handle: workspace_group,
                outputs: Vec::new(),
                members: Vec::new(),
                removed: false,
            });
        }
        ext_workspace_manager_v1::Event::Workspace { workspace } => {
            listed.workspaces.push(Workspace {
                handle: workspace,
                name: String::new(),
                coordinates: Vec::new(),
                active: false,
                capabilities: WorkspaceCapabilities::empty(),
                removed: false,
            });
        }
        ext_workspace_manager_v1::Event::Done => listed.done += 1,
        _ => {}
    }
}

fn on_ext_workspace_group_handle_v1(
    listed: &mut Listed,
    handle: ExtWorkspaceGroupHandleV1,
    event: ext_workspace_group_handle_v1::Event,
) {
    let Some(group) = listed.group(&handle) else {
        return;
    };
    match event {
        ext_workspace_group_handle_v1::Event::OutputEnter { output } => {
            group.outputs.push(output);
        }
        ext_workspace_group_handle_v1::Event::WorkspaceEnter { workspace } => {
            group.members.push(workspace);
        }
        ext_workspace_group_handle_v1::Event::WorkspaceLeave { workspace } => {
            group.members.retain(|member| *member != workspace);
        }
        ext_workspace_group_handle_v1::Event::Removed => group.removed = true,
        _ => {}
    }
}

fn on_ext_workspace_handle_v1(
    listed: &mut Listed,
    handle: ExtWorkspaceHandleV1,
    event: ext_workspace_handle_v1::Event,
) {
    let Some(workspace) = listed.workspace(&handle) else {
        return;
    };
    match event {
        ext_workspace_handle_v1::Event::Name { name } => workspace.name = name,
        ext_workspace_handle_v1::Event::Coordinates { coordinates } => {
            workspace.coordinates = coordinates
                .as_chunks::<4>()
                .0
                .iter()
                .map(|bytes| u32::from_ne_bytes(*bytes))
                .collect();
        }
        ext_workspace_handle_v1::Event::State { state } => {
            workspace.active = state.contains(ext_workspace_handle_v1::State::ACTIVE);
        }
        ext_workspace_handle_v1::Event::Capabilities { capabilities } => {
            workspace.capabilities = capabilities
        }
        ext_workspace_handle_v1::Event::Removed => workspace.removed = true,
        _ => {}
    }
}

fn lister(harness: &Harness) -> Client<Listed> {
    Client::connect_to(harness.socket(), |queue| {
        queue.on::<WlOutput>(on_wl_output);
        queue.on::<ExtWorkspaceManagerV1>(on_ext_workspace_manager_v1);
        queue.on::<ExtWorkspaceGroupHandleV1>(on_ext_workspace_group_handle_v1);
        queue.on::<ExtWorkspaceHandleV1>(on_ext_workspace_handle_v1);
    })
    .with_patience(PATIENCE)
}

fn showing(harness: &Harness) -> usize {
    let state: State = harness.ask(Request::new(CAPABILITY, STATE));
    state.displays[0].showing
}

#[test]
fn each_output_lists_ten_workspaces() {
    let harness = Harness::full(EXTENT, &[]);
    let mut lister = lister(&harness);
    let output: WlOutput = lister.global(4);
    let _manager: ExtWorkspaceManagerV1 = lister.global(1);
    lister.until("first workspace list", |listed| listed.done > 0);

    let listed = &lister.received;
    assert_eq!(listed.groups.len(), 1);
    let group = &listed.groups[0];
    assert_eq!(group.outputs, [output]);
    assert_eq!(group.members.len(), 10);
    let names: Vec<&str> = listed
        .workspaces
        .iter()
        .map(|workspace| workspace.name.as_str())
        .collect();
    assert_eq!(names, ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"]);
    let coordinates: Vec<Vec<u32>> = listed
        .workspaces
        .iter()
        .map(|workspace| workspace.coordinates.clone())
        .collect();
    assert_eq!(coordinates, (0..10).map(|at| vec![at]).collect::<Vec<_>>());
    assert!(
        listed
            .workspaces
            .iter()
            .all(|workspace| workspace.capabilities == WorkspaceCapabilities::ACTIVATE)
    );
    assert_eq!(listed.active(), ["1"]);
}

#[test]
fn workspace_activation_updates_active_state() {
    let harness = Harness::full(EXTENT, &[]);
    let mut lister = lister(&harness);
    let manager: ExtWorkspaceManagerV1 = lister.global(1);
    lister.until("first workspace list", |listed| listed.done > 0);

    lister.received.named("3").activate();
    manager.commit();
    lister.until("workspace 3 active", |listed| listed.active() == ["3"]);
    assert_eq!(showing(&harness), 3);

    lister.received.named("7").deactivate();
    lister.received.named("7").remove();
    manager.commit();
    lister.roundtrip();
    assert_eq!(lister.received.active(), ["3"]);

    let _: () =
        harness.ask(Request::new(CAPABILITY, SHOW).encode(&serde_json::json!({ "number": 5 })));
    lister.until("workspace 5 active", |listed| listed.active() == ["5"]);
}
