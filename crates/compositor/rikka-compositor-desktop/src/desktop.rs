use std::num::NonZeroUsize;

use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_compositor_shell::{LayerId, PopupId, ShellRole, ToplevelId};
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::{LayoutView, OutputId, XWindowId};
use rustc_hash::FxHashMap;
use slotmap::{SecondaryMap, SlotMap};

use crate::error::{Error, Result};
use crate::ids::WindowId;
use crate::layer::Layer;
use crate::layers::Levels;
use crate::lock::Lock;
use crate::new_window::NewWindow;
use crate::output::OutputState;
use crate::popup::Popup;
use crate::target::Target;
use crate::visibility::Visibility;
use crate::window::{Window, WindowKind};
use crate::workspace::Workspace;

#[derive(Debug)]
pub struct Desktop<P = ()> {
    pub(crate) windows: SlotMap<WindowId, Window>,
    pub(crate) policies: SecondaryMap<WindowId, P>,
    pub(crate) toplevels: SecondaryMap<ToplevelId, WindowId>,
    pub(crate) x11: FxHashMap<XWindowId, WindowId>,
    pub(crate) roots: SecondaryMap<SurfaceId, WindowId>,
    pub(crate) stack: Vec<WindowId>,
    pub(crate) unmanaged_stack: Vec<WindowId>,
    pub(crate) window_popups: SecondaryMap<WindowId, Vec<PopupId>>,
    pub(crate) outputs: Vec<(OutputId, OutputState)>,
    pub(crate) layouts: LayoutView,
    pub(crate) levels: SecondaryMap<OutputId, Levels>,
    pub(crate) workspaces: NonZeroUsize,
    pub(crate) layers: SecondaryMap<LayerId, Layer>,
    pub(crate) layer_order: Vec<LayerId>,
    pub(crate) layer_sequence: u64,
    pub(crate) layer_popups: SecondaryMap<LayerId, Vec<PopupId>>,
    pub(crate) popups: SecondaryMap<PopupId, Popup>,
    pub(crate) lock: Lock,
}

impl<P> Desktop<P> {
    #[must_use]
    pub fn new(workspaces: NonZeroUsize, layouts: LayoutView) -> Self {
        Self {
            windows: SlotMap::with_key(),
            policies: SecondaryMap::new(),
            toplevels: SecondaryMap::new(),
            x11: FxHashMap::default(),
            roots: SecondaryMap::new(),
            stack: Vec::new(),
            unmanaged_stack: Vec::new(),
            window_popups: SecondaryMap::new(),
            outputs: Vec::new(),
            layouts,
            levels: SecondaryMap::new(),
            workspaces,
            layers: SecondaryMap::new(),
            layer_order: Vec::new(),
            layer_sequence: 0,
            layer_popups: SecondaryMap::new(),
            popups: SecondaryMap::new(),
            lock: Lock::default(),
        }
    }

    pub fn add_window(&mut self, new: NewWindow) -> Result<WindowId>
    where
        P: Default,
    {
        if self.window_for_kind(new.kind).is_some() {
            return Err(Error::DuplicateWindow(new.kind));
        }
        let workspace = match (new.unmanaged, new.workspace) {
            (true, _) => None,
            (false, Some(workspace)) => Some(self.checked(workspace)?),
            (false, None) => self.home_for(centre(new.geometry)).map(|output| {
                let showing = self
                    .output_record(output)
                    .map_or(1, |record| record.showing);
                Workspace::new(output, showing)
            }),
        };
        let mut window = Window::new(new.kind, new.surface, new.geometry, new.unmanaged);
        window.workspace = workspace;
        let id = self.windows.insert(window);
        self.policies.insert(id, P::default());
        match new.kind {
            WindowKind::Xdg(toplevel) => {
                self.toplevels.insert(toplevel, id);
            }
            WindowKind::X11(x11) => {
                self.x11.insert(x11, id);
            }
        }
        if let Some(surface) = new.surface {
            self.roots.insert(surface, id);
        }
        if new.unmanaged {
            self.unmanaged_stack.push(id);
        } else {
            self.stack.push(id);
        }
        self.window_popups.insert(id, Vec::new());
        self.settle(id);
        Ok(id)
    }

    pub fn remove_window(&mut self, id: WindowId) -> Option<(Window, P)> {
        let window = self.windows.remove(id)?;
        let policy = self.policies.remove(id)?;
        self.forget_kind(id, window.kind);
        self.forget_root(id, window.surface);
        self.stack.retain(|stacked| *stacked != id);
        self.unmanaged_stack.retain(|stacked| *stacked != id);
        if let Some(popups) = self.window_popups.remove(id) {
            self.drop_popups(popups);
        }
        Some((window, policy))
    }

    pub fn window(&self, id: WindowId) -> Option<&Window> {
        self.windows.get(id)
    }

    pub fn window_mut(&mut self, id: WindowId) -> Option<&mut Window> {
        self.windows.get_mut(id)
    }

    pub fn policy(&self, id: WindowId) -> Option<&P> {
        self.policies.get(id)
    }

    pub fn policy_mut(&mut self, id: WindowId) -> Option<&mut P> {
        self.policies.get_mut(id)
    }

    pub fn window_and_policy_mut(&mut self, id: WindowId) -> Option<(&mut Window, &mut P)> {
        Some((self.windows.get_mut(id)?, self.policies.get_mut(id)?))
    }

    pub fn windows(&self) -> impl Iterator<Item = (WindowId, &Window)> {
        self.windows.iter()
    }

    pub fn policies(&self) -> impl Iterator<Item = (WindowId, &P)> {
        self.policies.iter()
    }

    pub fn window_for_toplevel(&self, toplevel: ToplevelId) -> Option<WindowId> {
        self.toplevels.get(toplevel).copied()
    }

    pub fn window_for_x11(&self, window: XWindowId) -> Option<WindowId> {
        self.x11.get(&window).copied()
    }

    pub fn window_for_surface(&self, surface: SurfaceId) -> Option<WindowId> {
        self.roots.get(surface).copied()
    }

    pub fn associate(&mut self, id: WindowId, surface: Option<SurfaceId>) -> Result<()> {
        let window = self.known_mut(id)?;
        if matches!(window.visibility, Visibility::Exiting(_)) {
            return Err(Error::Exiting(id));
        }
        let before = std::mem::replace(&mut window.surface, surface);
        self.forget_root(id, before);
        if let Some(surface) = surface {
            self.roots.insert(surface, id);
        }
        Ok(())
    }

    pub fn target_for_role(&self, role: ShellRole) -> Option<Target> {
        match role {
            ShellRole::Toplevel(toplevel) => self.window_for_toplevel(toplevel).map(Target::Window),
            ShellRole::Popup(popup) => self
                .popups
                .contains_key(popup)
                .then_some(Target::Popup(popup)),
            ShellRole::Layer(layer) => self
                .layers
                .contains_key(layer)
                .then_some(Target::Layer(layer)),
            ShellRole::LockSurface(lock) => self
                .lock
                .surfaces
                .contains_key(lock)
                .then_some(Target::Lock(lock)),
        }
    }

    pub(crate) fn window_for_kind(&self, kind: WindowKind) -> Option<WindowId> {
        match kind {
            WindowKind::Xdg(toplevel) => self.window_for_toplevel(toplevel),
            WindowKind::X11(x11) => self.window_for_x11(x11),
        }
    }

    pub(crate) fn forget_kind(&mut self, id: WindowId, kind: WindowKind) {
        match kind {
            WindowKind::Xdg(toplevel) => {
                if self.toplevels.get(toplevel) == Some(&id) {
                    self.toplevels.remove(toplevel);
                }
            }
            WindowKind::X11(x11) => {
                if self.x11.get(&x11) == Some(&id) {
                    self.x11.remove(&x11);
                }
            }
        }
    }

    pub(crate) fn forget_root(&mut self, id: WindowId, surface: Option<SurfaceId>) {
        if let Some(surface) = surface
            && self.roots.get(surface) == Some(&id)
        {
            self.roots.remove(surface);
        }
    }

    pub(crate) fn known(&self, id: WindowId) -> Result<&Window> {
        self.windows.get(id).ok_or(Error::UnknownWindow(id))
    }

    pub(crate) fn known_mut(&mut self, id: WindowId) -> Result<&mut Window> {
        self.windows.get_mut(id).ok_or(Error::UnknownWindow(id))
    }

    pub(crate) fn checked(&self, workspace: Workspace) -> Result<Workspace> {
        self.output_record(workspace.output())
            .ok_or(Error::UnknownOutput(workspace.output()))?;
        self.check_number(workspace.number())?;
        Ok(workspace)
    }

    pub(crate) fn check_number(&self, number: usize) -> Result<()> {
        if (1..=self.workspaces.get()).contains(&number) {
            return Ok(());
        }
        Err(Error::NoSuchWorkspace {
            number,
            count: self.workspaces.get(),
        })
    }

    pub(crate) fn home_for(&self, point: Point<i32, Logical>) -> Option<OutputId> {
        self.outputs()
            .find(|(_, output)| output.geometry().contains(point))
            .map(|(id, _)| id)
            .or_else(|| self.outputs.first().map(|(id, _)| *id))
    }

    pub(crate) fn is_showing(&self, workspace: Workspace) -> bool {
        self.output_record(workspace.output())
            .is_some_and(|output| output.showing == workspace.number())
    }

    pub(crate) fn settle(&mut self, id: WindowId) {
        let Some(window) = self.windows.get(id) else {
            return;
        };
        if matches!(
            window.visibility,
            Visibility::Minimized | Visibility::Exiting(_)
        ) {
            return;
        }
        let visibility = match window.workspace {
            Some(workspace) if !self.is_showing(workspace) => Visibility::OnHiddenWorkspace,
            _ => Visibility::Shown,
        };
        if let Some(window) = self.windows.get_mut(id) {
            window.visibility = visibility;
        }
    }

    pub(crate) fn on_screen(&self, window: &Window) -> bool {
        window.drawable()
            && window
                .workspace
                .is_none_or(|workspace| self.is_showing(workspace))
    }
}

fn centre(rect: Rect<i32, Logical>) -> Point<i32, Logical> {
    rect.loc + Point::new(rect.size.w / 2, rect.size.h / 2)
}
