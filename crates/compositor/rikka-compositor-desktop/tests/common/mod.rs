use std::collections::HashMap;
use std::num::NonZeroUsize;

use rikka_compositor_desktop::{
    Arrangement, Desktop, Drawn, LayerOrder, NewWindow, SceneLayers, SurfaceTrees, WindowId,
    WindowKind, Workspace,
};
use rikka_compositor_geometry::{Logical, Point, Rect, Size, Transform};
use rikka_compositor_protocols::zwlr_layer_shell_v1::Layer as Level;
use rikka_compositor_protocols::zwlr_layer_surface_v1::{Anchor, KeyboardInteractivity};
use rikka_compositor_shell::{
    LayerId, LayerState, LockSurfaceId, Margins, PopupId, PopupParent, ToplevelId,
};
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::{Layouts, OutputId, OutputLayout};
use slotmap::SlotMap;

pub(crate) fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect<i32, Logical> {
    Rect::new(Point::new(x, y), Size::new(w, h))
}

pub(crate) fn point(x: f64, y: f64) -> Point<f64, Logical> {
    Point::new(x, y)
}

pub(crate) fn layer_state(
    level: Level,
    anchor: Anchor,
    exclusive_zone: i32,
    w: i32,
    h: i32,
) -> LayerState {
    LayerState {
        layer: level,
        anchor,
        exclusive_zone,
        exclusive_edge: Anchor::empty(),
        margins: Margins::default(),
        keyboard_interactivity: KeyboardInteractivity::None,
        size: Size::new(w, h),
    }
}

#[derive(Debug, Clone, Copy)]
struct Tree {
    extent: Size<i32, Logical>,
    input: Rect<i32, Logical>,
}

#[derive(Debug, Default)]
pub(crate) struct Trees(HashMap<SurfaceId, Tree>);

impl Trees {
    pub(crate) fn set(&mut self, surface: SurfaceId, extent: Size<i32, Logical>) {
        self.0.insert(
            surface,
            Tree {
                extent,
                input: Rect::from_size(extent),
            },
        );
    }

    pub(crate) fn set_input(&mut self, surface: SurfaceId, input: Rect<i32, Logical>) {
        if let Some(tree) = self.0.get_mut(&surface) {
            tree.input = input;
        }
    }
}

impl SurfaceTrees for Trees {
    fn surface_under(
        &self,
        root: SurfaceId,
        point: Point<f64, Logical>,
    ) -> Option<(SurfaceId, Point<f64, Logical>)> {
        let tree = self.0.get(&root)?;
        tree.input.to_f64().contains(point).then_some((root, point))
    }

    fn bounding_box(&self, root: SurfaceId) -> Option<Rect<i32, Logical>> {
        self.0.get(&root).map(|tree| Rect::from_size(tree.extent))
    }
}

#[derive(Debug)]
pub(crate) struct World {
    pub(crate) desktop: Desktop,
    pub(crate) trees: Trees,
    layouts: Layouts,
    surfaces: SlotMap<SurfaceId, ()>,
    toplevels: SlotMap<ToplevelId, ()>,
    outputs: SlotMap<OutputId, ()>,
    layers: SlotMap<LayerId, ()>,
    popups: SlotMap<PopupId, ()>,
    locks: SlotMap<LockSurfaceId, ()>,
}

impl World {
    pub(crate) fn new(workspaces: usize) -> Self {
        let layouts = Layouts::default();
        Self {
            desktop: Desktop::new(
                NonZeroUsize::new(workspaces).expect("some workspaces"),
                layouts.view(),
            ),
            trees: Trees::default(),
            layouts,
            surfaces: SlotMap::with_key(),
            toplevels: SlotMap::with_key(),
            outputs: SlotMap::with_key(),
            layers: SlotMap::with_key(),
            popups: SlotMap::with_key(),
            locks: SlotMap::with_key(),
        }
    }

    pub(crate) fn output(&mut self, geometry: Rect<i32, Logical>) -> OutputId {
        let id = self.outputs.insert(());
        self.layouts.set(id, layout(geometry));
        self.desktop.add_output(id).expect("a new display");
        id
    }

    pub(crate) fn place_output(&mut self, output: OutputId, geometry: Rect<i32, Logical>) {
        self.layouts.set(output, layout(geometry));
    }

    pub(crate) fn surface(&mut self, extent: Size<i32, Logical>) -> SurfaceId {
        let surface = self.surfaces.insert(());
        self.trees.set(surface, extent);
        surface
    }

    pub(crate) fn toplevel(&mut self) -> ToplevelId {
        self.toplevels.insert(())
    }

    pub(crate) fn new_window(&mut self, geometry: Rect<i32, Logical>) -> NewWindow {
        let surface = self.surface(geometry.size);
        let toplevel = self.toplevel();
        NewWindow::xdg(toplevel, surface, geometry)
    }

    pub(crate) fn window(&mut self, geometry: Rect<i32, Logical>) -> WindowId {
        let new = self.new_window(geometry);
        self.desktop.add_window(new).expect("a new window")
    }

    pub(crate) fn window_on(
        &mut self,
        geometry: Rect<i32, Logical>,
        workspace: Workspace,
    ) -> WindowId {
        let new = self.new_window(geometry).with_workspace(workspace);
        self.desktop.add_window(new).expect("a new window")
    }

    pub(crate) fn unmanaged_window(&mut self, geometry: Rect<i32, Logical>) -> WindowId {
        let new = self.new_window(geometry).unmanaged();
        self.desktop.add_window(new).expect("a new window")
    }

    pub(crate) fn toplevel_of(&self, window: WindowId) -> ToplevelId {
        match self.desktop.window(window).expect("a window").kind() {
            WindowKind::Xdg(toplevel) => toplevel,
            WindowKind::X11(_) => panic!("an X11 window has no toplevel"),
        }
    }

    pub(crate) fn layer(&mut self, output: OutputId, state: LayerState) -> LayerId {
        self.ordered_layer(output, state, LayerOrder::Created)
    }

    pub(crate) fn ordered_layer(
        &mut self,
        output: OutputId,
        state: LayerState,
        order: LayerOrder,
    ) -> LayerId {
        let id = self.layers.insert(());
        let surface = self.surfaces.insert(());
        self.desktop
            .add_layer(id, surface, output, state, order)
            .expect("a new layer");
        id
    }

    pub(crate) fn arrange(&mut self, output: OutputId) -> Arrangement {
        let arrangement = self.desktop.arrange(output).expect("a known display");
        let sized: Vec<_> = self
            .desktop
            .layers_on(output)
            .map(|(_, layer)| (layer.surface(), layer.geometry().size))
            .collect();
        for (surface, size) in sized {
            self.trees.set(surface, size);
        }
        arrangement
    }

    pub(crate) fn popup(&mut self, parent: PopupParent, geometry: Rect<i32, Logical>) -> PopupId {
        let id = self.popups.insert(());
        let surface = self.surface(geometry.size);
        self.desktop
            .add_popup(id, parent, surface)
            .expect("a new popup");
        self.desktop
            .popup_mut(id)
            .expect("the popup")
            .set_geometry(geometry);
        id
    }

    pub(crate) fn drawn(&self, output: OutputId, around: SceneLayers) -> Vec<Drawn> {
        let mut drawn = Vec::new();
        self.desktop
            .drawn_into(&self.trees, output, around, &mut drawn);
        drawn
    }

    pub(crate) fn lock_surface(&mut self, output: OutputId) -> (LockSurfaceId, SurfaceId) {
        let id = self.locks.insert(());
        let size = self
            .desktop
            .output(output)
            .expect("a display")
            .geometry()
            .size;
        let surface = self.surface(size);
        self.desktop
            .add_lock_surface(id, output, surface)
            .expect("a new lock surface");
        (id, surface)
    }
}

pub(crate) fn layout(geometry: Rect<i32, Logical>) -> OutputLayout {
    OutputLayout {
        geometry,
        scale: 1.0,
        transform: Transform::Normal,
    }
}
