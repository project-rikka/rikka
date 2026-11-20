mod announce;
mod around;
mod arrange;
mod arrive;
mod caption_paint;
mod caption_pointer;
mod columns;
mod committed;
mod configures;
mod decoration;
mod displays;
mod enlarge;
mod enlargement;
mod family;
mod focus;
mod focus_loans;
mod follow;
mod hidden_pace;
mod limits;
mod minimize;
mod mode;
mod motion;
mod output_anchors;
mod portrayed;
mod record;
mod shape;
mod stacking;
mod stretch;
mod strip_swipe;
mod tile_drag;
mod tiling;
mod workspaces;

use std::collections::HashMap;
use std::fmt;
use std::num::NonZeroUsize;
use std::time::Instant;

use rikka_compositor_desktop::{
    Desktop, Placement, Target, Window, WindowId, WindowKind, Workspace,
};
use rikka_compositor_geometry::{Logical, Point, Rect, Size};
use rikka_compositor_reactor::Token;
use rikka_compositor_recency::Recency;
use rikka_compositor_scrolling::{Frame, Spacing};
use rikka_compositor_shell::{LayerId, ShellRole, ToplevelId, shell_role};
use rikka_compositor_surface::{SurfaceId, Surfaces};
use rikka_compositor_types::{LayoutView, OutputId};
use rikka_session::profile::Profile;
use rikka_session::workspaces::COUNT;

pub(crate) use around::{around, drawn_on, fullscreen_surface, overviewed};
pub(crate) use arrive::Arrival;
pub(crate) use caption_paint::ButtonLight;
pub(crate) use columns::ColumnCommand;
pub(crate) use decoration::{Decoration, frame_extents};
pub(crate) use enlargement::{Enlargement, Enlargements};
pub(crate) use hidden_pace::hidden_frame_pace;
pub(crate) use mode::Snapped;
pub(crate) use portrayed::portrayed;
pub(crate) use record::Record;
pub(crate) use stacking::XStacking;
pub(crate) use stretch::{Anchor, gripped};

use crate::policy::{LayoutMode, WindowRules};
use crate::render::WindowShape;
use caption_pointer::CaptionPointer;
use focus_loans::FocusLoans;
use tiling::{Kind, Tiling};

fn fallback_area() -> Rect<i32, Logical> {
    Rect::from_size(Size::new(800, 600))
}

fn window_shape(record: &Window) -> Option<WindowShape> {
    let rounded = match record.placement() {
        Placement::Floating | Placement::Tiled => true,
        Placement::Maximized | Placement::Fullscreen => false,
    };
    (!record.unmanaged()).then(|| WindowShape::at_rest(record.geometry(), rounded))
}

pub(crate) struct Windows {
    pub(crate) desktop: Desktop<Record>,
    pub(crate) tiling: Tiling,
    pub(crate) swiped: Option<Workspace>,
    pub(crate) pending_configures: Vec<WindowId>,
    pub(crate) patience: Option<(Instant, Token)>,
    pub(crate) overviews: Vec<LayerId>,
    pub(crate) scrims: Vec<LayerId>,
    pub(crate) stacking: XStacking,
    pub(crate) arrivals: HashMap<ToplevelId, Arrival>,
    pub(crate) changed: bool,
    pub(crate) focus_loans: FocusLoans,
    pub(crate) recency: Recency<WindowId>,
    pub(crate) captions: CaptionPointer,
    rules: WindowRules,
}

impl Windows {
    pub(crate) fn new(profile: Profile, rules: WindowRules, layouts: LayoutView) -> Self {
        let workspaces = match profile {
            Profile::Full => COUNT,
            Profile::Simple => 1,
        };
        Self {
            desktop: Desktop::new(
                NonZeroUsize::new(workspaces).unwrap_or(NonZeroUsize::MIN),
                layouts,
            ),
            tiling: Tiling::new(Kind::from_mode(rules.mode)),
            swiped: None,
            pending_configures: Vec::new(),
            patience: None,
            overviews: Vec::new(),
            scrims: Vec::new(),
            stacking: XStacking::default(),
            arrivals: HashMap::new(),
            changed: true,
            focus_loans: FocusLoans::default(),
            recency: Recency::default(),
            captions: CaptionPointer::default(),
            rules,
        }
    }

    pub(crate) fn forget_layer(&mut self, layer: LayerId) {
        self.overviews.retain(|overview| *overview != layer);
        self.scrims.retain(|scrim| *scrim != layer);
    }

    pub(crate) fn rules(&self) -> WindowRules {
        self.rules
    }

    pub(crate) fn tiling_mode(&self) -> bool {
        self.rules.mode.tiles()
    }

    pub(crate) fn scrolling_mode(&self) -> bool {
        self.rules.mode == LayoutMode::Scrolling
    }

    pub(crate) fn scrolls(&self, window: WindowId) -> bool {
        self.tiling.scrolls(window)
    }

    pub(crate) fn scrolls_on(&self, output: OutputId) -> bool {
        self.showing(output)
            .and_then(|workspace| self.tiling.strip(workspace))
            .is_some_and(|strip| !strip.is_empty())
    }

    pub(crate) fn frame(&self, output: OutputId) -> Frame {
        let gaps = self.rules.gaps;
        Frame {
            work: self.usable_area(output),
            full: self.full_area(output),
            spacing: Spacing {
                inner: gaps.inner,
                outer: gaps.outer,
            },
        }
    }

    pub(crate) fn queue_configure(&mut self, window: WindowId) {
        if !self.pending_configures.contains(&window) {
            self.pending_configures.push(window);
        }
    }

    pub(crate) fn window_of(&self, surfaces: &Surfaces, surface: SurfaceId) -> Option<WindowId> {
        let root = surfaces.root(surface);
        match shell_role(surfaces, root) {
            Some(ShellRole::Toplevel(toplevel)) => self.desktop.window_for_toplevel(toplevel),
            Some(role) => match self.desktop.root_of(self.desktop.target_for_role(role)?) {
                Target::Window(window) => Some(window),
                _ => None,
            },
            None => self.desktop.window_for_surface(root),
        }
    }

    pub(crate) fn target_of(&self, surfaces: &Surfaces, root: SurfaceId) -> Option<Target> {
        shell_role(surfaces, root)
            .and_then(|role| self.desktop.target_for_role(role))
            .or_else(|| self.window_of(surfaces, root).map(Target::Window))
    }

    pub(crate) fn drawable(&self, surfaces: &Surfaces, root: SurfaceId) -> bool {
        let target = self.target_of(surfaces, root);
        match target {
            Some(Target::Window(window)) => self
                .desktop
                .window(window)
                .is_some_and(|record| record.visibility().shown()),
            Some(Target::Layer(layer)) => self
                .desktop
                .layer(layer)
                .is_some_and(|layer| layer.mapped()),
            Some(Target::Popup(_) | Target::Lock(_)) => true,
            None => false,
        }
    }

    pub(crate) fn usable_area(&self, output: OutputId) -> Rect<i32, Logical> {
        self.desktop
            .output(output)
            .map_or_else(fallback_area, |output| output.work_area())
    }

    pub(crate) fn full_area(&self, output: OutputId) -> Rect<i32, Logical> {
        self.desktop
            .output(output)
            .map_or_else(fallback_area, |output| output.geometry())
    }

    pub(crate) fn output_at(&self, point: Point<f64, Logical>) -> Option<OutputId> {
        self.desktop
            .output_at(point)
            .or_else(|| self.desktop.outputs().next().map(|(id, _)| id))
    }

    pub(crate) fn output_of(&self, window: WindowId) -> Option<OutputId> {
        let window = self.desktop.window(window)?;
        window.output().or_else(|| {
            let centre = window.geometry().loc.to_f64()
                + Point::new(
                    f64::from(window.geometry().size.w) / 2.0,
                    f64::from(window.geometry().size.h) / 2.0,
                );
            self.output_at(centre)
        })
    }

    pub(crate) fn showing(&self, output: OutputId) -> Option<Workspace> {
        self.desktop.showing(output)
    }

    pub(crate) fn is_decorated(&self, window: WindowId) -> bool {
        self.desktop
            .policy(window)
            .is_some_and(|policy| policy.decorated.is_some())
    }

    pub(crate) fn is_tiled(&self, window: WindowId) -> bool {
        self.tiling.contains(window)
    }

    pub(crate) fn outlined(&self) -> Option<WindowId> {
        if !self.rules.focus_outline {
            return None;
        }
        let window = self.recency.current()?;
        let shown = self
            .desktop
            .window(window)
            .is_some_and(|record| record.visibility().shown());
        (shown && self.is_tiled(window) && !self.enlargements(window).any()).then_some(window)
    }

    pub(crate) fn shape(&self, window: WindowId) -> Option<WindowShape> {
        window_shape(self.desktop.window(window)?)
    }

    pub(crate) fn is_x11(&self, window: WindowId) -> bool {
        self.desktop
            .window(window)
            .is_some_and(|window| matches!(window.kind(), WindowKind::X11(_)))
    }

    pub(crate) fn is_x11_target(&self, target: Target) -> bool {
        match self.desktop.root_of(target) {
            Target::Window(window) => self.is_x11(window),
            Target::Layer(_) | Target::Popup(_) | Target::Lock(_) => false,
        }
    }

    pub(crate) fn enlargements(&self, window: WindowId) -> Enlargements {
        self.desktop
            .policy(window)
            .map_or_else(Enlargements::default, |policy| policy.mode.enlargements())
    }
}

impl fmt::Debug for Windows {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Windows")
            .field("windows", &self.desktop.windows().count())
            .field("tiled", &self.tiling)
            .field("pending_configures", &self.pending_configures)
            .field("rules", &self.rules)
            .finish_non_exhaustive()
    }
}
