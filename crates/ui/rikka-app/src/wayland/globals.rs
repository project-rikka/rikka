use rikka_wayland::{Display, FractionalScaleManager};
use rikka_wayland_client::Route;
use rikka_wayland_client::wl_compositor::WlCompositor;
use rikka_wayland_client::xdg_wm_base::XdgWmBase;
use rikka_wayland_client::xdg_wm_dialog_v1::XdgWmDialogV1;
use rikka_wayland_client::zwp_pointer_constraints_v1::ZwpPointerConstraintsV1;
use rikka_wayland_client::zxdg_decoration_manager_v1::ZxdgDecorationManagerV1;
use rikka_wayland_client::zxdg_exporter_v2::ZxdgExporterV2;
use rikka_wayland_client::zxdg_importer_v2::ZxdgImporterV2;
use rikka_wayland_client::zxdg_output_manager_v1::ZxdgOutputManagerV1;

use crate::error::Result;

#[derive(Clone, Debug)]
pub(crate) struct Globals {
    pub(crate) compositor: WlCompositor,
    pub(crate) wm_base: XdgWmBase,
    pub(crate) decorations: Option<ZxdgDecorationManagerV1>,
    pub(crate) fractional_scale: Option<FractionalScaleManager>,
    pub(crate) constraints: Option<ZwpPointerConstraintsV1>,
    pub(crate) described_outputs: Option<ZxdgOutputManagerV1>,
    pub(crate) exporter: Option<ZxdgExporterV2>,
    pub(crate) importer: Option<ZxdgImporterV2>,
    pub(crate) dialogs: Option<XdgWmDialogV1>,
}

impl Globals {
    pub(crate) fn bind(display: &Display, route: &Route) -> Result<Globals> {
        Ok(Globals {
            compositor: display.bind_on(route, 3..=6)?,
            wm_base: display.bind_on(route, 1..=6)?,
            decorations: display.bind_optional_on(route, 1..=1),
            fractional_scale: FractionalScaleManager::bind(display, route),
            constraints: display.bind_optional_on(route, 1..=1),
            described_outputs: display.bind_optional_on(route, 1..=3),
            exporter: display.bind_optional_on(route, 1..=1),
            importer: display.bind_optional_on(route, 1..=1),
            dialogs: display.bind_optional_on(route, 1..=1),
        })
    }
}
