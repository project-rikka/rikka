use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_compositor_protocols::zwlr_layer_shell_v1::Layer as Level;
use rikka_compositor_shell::LayerId;
use rikka_compositor_types::OutputId;
use smallvec::SmallVec;

use crate::desktop::Desktop;
use crate::drawn::Drawn;
use crate::ids::WindowId;
use crate::layer::Layer;
use crate::popup::Parent;
use crate::scene_layers::SceneLayers;
use crate::target::Target;
use crate::trees::SurfaceTrees;
use crate::visibility::Visibility;
use crate::window::{Placement, Window};

impl<P> Desktop<P> {
    pub fn drawn_into(
        &self,
        trees: &impl SurfaceTrees,
        output: OutputId,
        around: SceneLayers,
        drawn: &mut Vec<Drawn>,
    ) {
        drawn.clear();
        let Some(record) = self.output_record(output) else {
            return;
        };
        if self.lock.locked {
            drawn.extend(
                self.lock_surface_on(output)
                    .map(|(lock, surface)| Drawn::Surface {
                        target: Target::Lock(lock),
                        root: surface,
                        location: record.geometry().loc,
                    }),
            );
            return;
        }
        if around.backdrop {
            self.draw_layers(drawn, output, Level::Background);
            self.draw_layers(drawn, output, Level::Bottom);
        }
        if around.windows {
            for id in self.windows_drawn_on(trees, output) {
                self.draw_window(drawn, id);
            }
        }
        if around.top {
            self.draw_layers(drawn, output, Level::Top);
        }
        self.draw_layers(drawn, output, Level::Overlay);
    }

    pub fn drawn_window_on_wallpapers_into(&self, id: WindowId, drawn: &mut Vec<Drawn>) {
        drawn.clear();
        let Some(window) = self.windows.get(id) else {
            return;
        };
        let area = window.geometry;
        for (output, _) in self
            .outputs()
            .filter(|(_, record)| record.geometry().overlaps(area))
        {
            drawn.extend(
                self.layers_at(output, Level::Background)
                    .map(|(layer, record)| layer_drawn(layer, record)),
            );
        }
        self.draw_window(drawn, id);
    }

    pub fn windows_drawn_on<'a>(
        &'a self,
        trees: &'a impl SurfaceTrees,
        output: OutputId,
    ) -> impl DoubleEndedIterator<Item = WindowId> + 'a {
        let area = self.output_record(output).map(|record| record.geometry());
        self.stacking().filter(move |id| {
            area.is_some_and(|area| {
                self.windows
                    .get(*id)
                    .is_some_and(|window| self.drawn_on(trees, window, output, area))
            })
        })
    }

    pub fn fullscreen_on(&self, output: OutputId) -> Option<WindowId> {
        let area = self.output_record(output)?.geometry();
        let topmost = self.stack.iter().rev().copied().find(|id| {
            self.windows.get(*id).is_some_and(|window| {
                window.visibility == Visibility::Shown && window.output() == Some(output)
            })
        })?;
        let window = self.windows.get(topmost)?;
        (window.placement == Placement::Fullscreen && window.geometry.contains_rect(area))
            .then_some(topmost)
    }

    pub(crate) fn drawn_on(
        &self,
        trees: &impl SurfaceTrees,
        window: &Window,
        output: OutputId,
        area: Rect<i32, Logical>,
    ) -> bool {
        let own = window.output() == Some(output);
        self.on_screen(window)
            && (own
                || !window.confined
                    && (window.geometry.overlaps(area) || bounds(trees, window).overlaps(area)))
    }

    fn draw_window(&self, drawn: &mut Vec<Drawn>, id: WindowId) {
        let Some(window) = self.windows.get(id) else {
            return;
        };
        if let Visibility::Exiting(snapshot) = window.visibility {
            drawn.push(Drawn::Snapshot {
                window: id,
                snapshot,
                geometry: window.geometry,
            });
            return;
        }
        let Some(surface) = window.surface else {
            return;
        };
        drawn.push(Drawn::Surface {
            target: Target::Window(id),
            root: surface,
            location: window.surface_location(),
        });
        self.draw_popups(drawn, Parent::Window(id), window.content().loc);
    }

    fn draw_layers(&self, drawn: &mut Vec<Drawn>, output: OutputId, level: Level) {
        for (id, layer) in self.layers_at(output, level) {
            drawn.push(layer_drawn(id, layer));
            self.draw_popups(drawn, Parent::Layer(id), layer.geometry.loc);
        }
    }

    fn draw_popups(&self, drawn: &mut Vec<Drawn>, parent: Parent, origin: Point<i32, Logical>) {
        let mut pending: SmallVec<[_; 8]> = self
            .popups_of(parent)
            .iter()
            .rev()
            .map(|id| (*id, origin))
            .collect();
        while let Some((id, origin)) = pending.pop() {
            let Some(popup) = self.popups.get(id) else {
                continue;
            };
            drawn.push(Drawn::Surface {
                target: Target::Popup(id),
                root: popup.surface,
                location: origin + popup.geometry.loc - popup.inset,
            });
            let inner = origin + popup.geometry.loc;
            pending.extend(popup.children.iter().rev().map(|child| (*child, inner)));
        }
    }
}

fn layer_drawn(id: LayerId, layer: &Layer) -> Drawn {
    Drawn::Surface {
        target: Target::Layer(id),
        root: layer.surface,
        location: layer.geometry.loc,
    }
}

fn bounds(trees: &impl SurfaceTrees, window: &Window) -> Rect<i32, Logical> {
    window
        .surface
        .and_then(|surface| trees.bounding_box(surface))
        .map(|tree| Rect::new(tree.loc + window.surface_location(), tree.size))
        .map_or(window.geometry, |tree| tree.merge(window.geometry))
}
