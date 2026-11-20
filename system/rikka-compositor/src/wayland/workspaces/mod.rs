mod list;

use rikka_compositor_protocols::ext_workspace_group_handle_v1::{self, ExtWorkspaceGroupHandleV1};
use rikka_compositor_protocols::ext_workspace_handle_v1::{self, ExtWorkspaceHandleV1};
use rikka_compositor_protocols::ext_workspace_manager_v1::{self, ExtWorkspaceManagerV1};
use rikka_compositor_server::{DisplayHandle, Module};

pub(crate) use list::WorkspaceList;

use crate::context::windows_ctx;
use crate::state::Compositor;

pub(crate) struct WorkspaceProtocol;

impl Module<Compositor> for WorkspaceProtocol {
    fn install(self, display: &DisplayHandle<Compositor>) {
        display
            .serve::<ExtWorkspaceManagerV1>(manager)
            .bound(bound)
            .destroyed(manager_destroyed);
        display
            .serve::<ExtWorkspaceGroupHandleV1>(group)
            .destroyed(group_destroyed);
        display
            .serve::<ExtWorkspaceHandleV1>(workspace)
            .destroyed(workspace_destroyed);
        display
            .global::<ExtWorkspaceManagerV1>()
            .privileged()
            .publish();
    }
}

fn bound(compositor: &mut Compositor, manager: ExtWorkspaceManagerV1) {
    compositor
        .workspace_list
        .bound(manager, compositor.outputs.wayland());
}

fn manager(
    compositor: &mut Compositor,
    manager: ExtWorkspaceManagerV1,
    request: ext_workspace_manager_v1::Request,
) {
    match request {
        ext_workspace_manager_v1::Request::Commit => {
            for workspace in compositor.workspace_list.commit(&manager) {
                windows_ctx!(compositor).show_workspace_on(workspace.output(), workspace.number());
            }
        }
        ext_workspace_manager_v1::Request::Stop => compositor.workspace_list.stop(&manager),
    }
}

fn manager_destroyed(compositor: &mut Compositor, manager: ExtWorkspaceManagerV1) {
    compositor.workspace_list.forget(&manager);
}

fn group(
    _: &mut Compositor,
    _: ExtWorkspaceGroupHandleV1,
    request: ext_workspace_group_handle_v1::Request,
) {
    match request {
        ext_workspace_group_handle_v1::Request::CreateWorkspace { .. }
        | ext_workspace_group_handle_v1::Request::Destroy => {}
    }
}

fn group_destroyed(compositor: &mut Compositor, group: ExtWorkspaceGroupHandleV1) {
    compositor.workspace_list.forget_group(&group);
}

fn workspace(
    compositor: &mut Compositor,
    workspace: ExtWorkspaceHandleV1,
    request: ext_workspace_handle_v1::Request,
) {
    match request {
        ext_workspace_handle_v1::Request::Activate => {
            compositor.workspace_list.activate(&workspace);
        }
        ext_workspace_handle_v1::Request::Deactivate
        | ext_workspace_handle_v1::Request::Assign { .. }
        | ext_workspace_handle_v1::Request::Remove
        | ext_workspace_handle_v1::Request::Destroy => {}
    }
}

fn workspace_destroyed(compositor: &mut Compositor, workspace: ExtWorkspaceHandleV1) {
    compositor.workspace_list.forget_workspace(&workspace);
}
