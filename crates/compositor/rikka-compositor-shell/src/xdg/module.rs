use rikka_compositor_protocols::xdg_activation_token_v1::XdgActivationTokenV1;
use rikka_compositor_protocols::xdg_activation_v1::XdgActivationV1;
use rikka_compositor_protocols::xdg_dialog_v1::XdgDialogV1;
use rikka_compositor_protocols::xdg_popup::XdgPopup;
use rikka_compositor_protocols::xdg_positioner::XdgPositioner;
use rikka_compositor_protocols::xdg_surface::XdgSurface;
use rikka_compositor_protocols::xdg_system_bell_v1::XdgSystemBellV1;
use rikka_compositor_protocols::xdg_toplevel::XdgToplevel;
use rikka_compositor_protocols::xdg_toplevel_drag_manager_v1::XdgToplevelDragManagerV1;
use rikka_compositor_protocols::xdg_toplevel_drag_v1::XdgToplevelDragV1;
use rikka_compositor_protocols::xdg_toplevel_icon_manager_v1::XdgToplevelIconManagerV1;
use rikka_compositor_protocols::xdg_toplevel_icon_v1::XdgToplevelIconV1;
use rikka_compositor_protocols::xdg_toplevel_tag_manager_v1::XdgToplevelTagManagerV1;
use rikka_compositor_protocols::xdg_wm_base::XdgWmBase;
use rikka_compositor_protocols::xdg_wm_dialog_v1::XdgWmDialogV1;
use rikka_compositor_protocols::zxdg_decoration_manager_v1::ZxdgDecorationManagerV1;
use rikka_compositor_protocols::zxdg_exported_v2::ZxdgExportedV2;
use rikka_compositor_protocols::zxdg_exporter_v2::ZxdgExporterV2;
use rikka_compositor_protocols::zxdg_imported_v2::ZxdgImportedV2;
use rikka_compositor_protocols::zxdg_importer_v2::ZxdgImporterV2;
use rikka_compositor_protocols::zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1;
use rikka_compositor_server::{DisplayHandle, Module};

use super::{
    activation, bell, decoration, dialog, drag, foreign, icon, popup_requests, positioner_requests,
    surface, tag, toplevel_requests, wm_base,
};
use crate::host::ShellHost;

#[derive(Debug, Clone, Default)]
#[must_use]
pub struct XdgShell {
    icon_sizes: Vec<i32>,
}

impl XdgShell {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn icon_sizes(mut self, sizes: impl IntoIterator<Item = i32>) -> Self {
        self.icon_sizes = sizes.into_iter().collect();
        self
    }
}

impl<S: ShellHost> Module<S> for XdgShell {
    fn install(self, display: &DisplayHandle<S>) {
        display
            .serve::<XdgWmBase>(wm_base::request)
            .bound(wm_base::bound)
            .destroyed(wm_base::destroyed);
        display
            .serve::<XdgPositioner>(positioner_requests::request)
            .destroyed(positioner_requests::destroyed);
        display
            .serve::<XdgSurface>(surface::request)
            .destroyed(surface::destroyed);
        display
            .serve::<XdgToplevel>(toplevel_requests::request)
            .destroyed(toplevel_requests::destroyed);
        display
            .serve::<XdgPopup>(popup_requests::request)
            .destroyed(popup_requests::destroyed);
        display.serve::<ZxdgDecorationManagerV1>(decoration::manager);
        display
            .serve::<ZxdgToplevelDecorationV1>(decoration::request)
            .destroyed(decoration::destroyed);
        display.serve::<XdgActivationV1>(activation::manager);
        display
            .serve::<XdgActivationTokenV1>(activation::request)
            .destroyed(activation::destroyed);
        display.serve::<ZxdgExporterV2>(foreign::exporter);
        display
            .serve::<ZxdgExportedV2>(foreign::exported)
            .destroyed(foreign::exported_destroyed);
        display.serve::<ZxdgImporterV2>(foreign::importer);
        display
            .serve::<ZxdgImportedV2>(foreign::imported)
            .destroyed(foreign::imported_destroyed);
        display.serve::<XdgWmDialogV1>(dialog::manager);
        display
            .serve::<XdgDialogV1>(dialog::request)
            .destroyed(dialog::destroyed);
        display.serve::<XdgToplevelIconManagerV1>(icon::manager);
        display
            .serve::<XdgToplevelIconV1>(icon::request)
            .destroyed(icon::destroyed);
        display.serve::<XdgToplevelTagManagerV1>(tag::request);
        display.serve::<XdgToplevelDragManagerV1>(drag::manager);
        display
            .serve::<XdgToplevelDragV1>(drag::request)
            .destroyed(drag::destroyed);
        display.serve::<XdgSystemBellV1>(bell::request);

        display.global::<XdgWmBase>().publish();
        display.global::<ZxdgDecorationManagerV1>().publish();
        display.global::<XdgActivationV1>().publish();
        display.global::<ZxdgExporterV2>().publish();
        display.global::<ZxdgImporterV2>().publish();
        display.global::<XdgWmDialogV1>().publish();
        display.global::<XdgToplevelTagManagerV1>().publish();
        display.global::<XdgToplevelDragManagerV1>().publish();
        display.global::<XdgSystemBellV1>().publish();
        let sizes = self.icon_sizes;
        display
            .global::<XdgToplevelIconManagerV1>()
            .on_bind(move |manager| icon::announce_sizes(manager, &sizes))
            .publish();
    }
}
