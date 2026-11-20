use std::iter::Rev;
use std::slice;

use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_protocols::zwlr_layer_shell_v1::Layer as Level;
use rikka_compositor_shell::PopupId;
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::OutputId;
use smallvec::SmallVec;

use crate::desktop::Desktop;
use crate::popup::Parent;
use crate::scene_layers::SceneLayers;
use crate::target::Target;
use crate::trees::SurfaceTrees;

pub(crate) struct VisitedPopup {
    pub(crate) target: Target,
    pub(crate) root: SurfaceId,
    pub(crate) location: Point<i32, Logical>,
}

enum Step<'a> {
    Children(Rev<slice::Iter<'a, PopupId>>, Point<i32, Logical>),
    Popup(VisitedPopup),
}

impl<P> Desktop<P> {
    pub(crate) fn walk_top_down<T>(
        &self,
        trees: &impl SurfaceTrees,
        output: OutputId,
        around: SceneLayers,
        visit: &mut impl FnMut(VisitedPopup) -> Option<T>,
    ) -> Option<T> {
        let record = self.output_record(output)?;
        if self.lock.locked {
            let (lock, root) = self.lock_surface_on(output)?;
            return visit(VisitedPopup {
                target: Target::Lock(lock),
                root,
                location: record.geometry().loc,
            });
        }
        let area = record.geometry();
        let found = self
            .walk_layers(output, Level::Overlay, visit)
            .or_else(|| {
                around
                    .top
                    .then(|| self.walk_layers(output, Level::Top, visit))
                    .flatten()
            })
            .or_else(|| {
                if !around.windows {
                    return None;
                }
                self.stacking().rev().find_map(|id| {
                    let window = self.windows.get(id)?;
                    if !self.drawn_on(trees, window, output, area) {
                        return None;
                    }
                    let root = window.surface?;
                    self.walk_popups(Parent::Window(id), window.content().loc, visit)
                        .or_else(|| {
                            visit(VisitedPopup {
                                target: Target::Window(id),
                                root,
                                location: window.surface_location(),
                            })
                        })
                })
            });
        found.or_else(|| {
            if !around.backdrop {
                return None;
            }
            self.walk_layers(output, Level::Bottom, visit)
                .or_else(|| self.walk_layers(output, Level::Background, visit))
        })
    }

    fn walk_layers<T>(
        &self,
        output: OutputId,
        level: Level,
        visit: &mut impl FnMut(VisitedPopup) -> Option<T>,
    ) -> Option<T> {
        self.layers_at(output, level).rev().find_map(|(id, layer)| {
            self.walk_popups(Parent::Layer(id), layer.geometry.loc, visit)
                .or_else(|| {
                    visit(VisitedPopup {
                        target: Target::Layer(id),
                        root: layer.surface,
                        location: layer.geometry.loc,
                    })
                })
        })
    }

    fn walk_popups<T>(
        &self,
        parent: Parent,
        origin: Point<i32, Logical>,
        visit: &mut impl FnMut(VisitedPopup) -> Option<T>,
    ) -> Option<T> {
        let mut steps: SmallVec<[Step<'_>; 8]> = SmallVec::new();
        steps.push(Step::Children(self.popups_of(parent).iter().rev(), origin));
        while let Some(step) = steps.last_mut() {
            match step {
                Step::Children(children, origin) => {
                    let origin = *origin;
                    let Some(id) = children.next() else {
                        steps.pop();
                        continue;
                    };
                    let Some(popup) = self.popups.get(*id) else {
                        continue;
                    };
                    steps.push(Step::Popup(VisitedPopup {
                        target: Target::Popup(*id),
                        root: popup.surface,
                        location: origin + popup.geometry.loc - popup.inset,
                    }));
                    steps.push(Step::Children(
                        popup.children.iter().rev(),
                        origin + popup.geometry.loc,
                    ));
                }
                Step::Popup(_) => {
                    let Some(Step::Popup(visited)) = steps.pop() else {
                        continue;
                    };
                    if let Some(found) = visit(visited) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }
}
