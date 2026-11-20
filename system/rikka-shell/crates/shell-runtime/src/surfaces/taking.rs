use rikka_blur::rectangles;
use rikka_types::{RoundedRect, Size};
use rikka_wayland_client::wl_compositor::WlCompositor;
use rikka_wayland_client::wl_surface::WlSurface;

use shell_kit::PointerMode;

pub(crate) type Area = [i32; 4];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Taking {
    Everything,
    Within(Vec<Area>),
}

pub(super) fn taking(mode: PointerMode, logical: Size, panes: &[Area]) -> Taking {
    match mode {
        PointerMode::Take => Taking::Everything,
        PointerMode::Ignore => Taking::Within(Vec::new()),
        PointerMode::Inset(inset) => Taking::Within(vec![[
            inset.left().round() as i32,
            inset.top().round() as i32,
            (logical.width - inset.horizontal()).round().max(0.0) as i32,
            (logical.height - inset.vertical()).round().max(0.0) as i32,
        ]]),
        PointerMode::Panes => Taking::Within(panes.to_vec()),
    }
}

pub(super) fn panes(glass: &[RoundedRect]) -> Vec<Area> {
    let mut areas = Vec::new();
    for pane in glass {
        rectangles(*pane, &mut areas);
    }
    areas
}

pub(super) fn take(compositor: &WlCompositor, surface: &WlSurface, taking: &Taking) {
    let Taking::Within(areas) = taking else {
        surface.set_input_region(None);
        return;
    };
    let region = compositor.create_region();
    for [x, y, width, height] in areas {
        region.add(*x, *y, *width, *height);
    }
    surface.set_input_region(Some(&region));
    region.destroy();
}
