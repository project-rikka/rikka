mod published;

use rikka_compositor_protocols::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1;
use rikka_compositor_protocols::ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1;
use rikka_compositor_server::{DisplayHandle, Module};

pub use published::ForeignToplevel;

pub(crate) use published::Published;

use crate::host::ShellHost;

#[derive(Debug, Clone, Copy, Default)]
pub struct ForeignToplevelList;

impl<S: ShellHost> Module<S> for ForeignToplevelList {
    fn install(self, display: &DisplayHandle<S>) {
        display
            .serve::<ExtForeignToplevelListV1>(published::list)
            .bound(published::bound)
            .destroyed(published::list_destroyed);
        display
            .serve::<ExtForeignToplevelHandleV1>(published::handle)
            .destroyed(published::handle_destroyed);
        display
            .global::<ExtForeignToplevelListV1>()
            .privileged()
            .publish();
    }
}
