use std::collections::HashMap;

use rikka_compositor_desktop::Drawn;
use rikka_compositor_geometry::Rect;
use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_surface::{PlacedSurface, SurfaceId, Surfaces};
use rikka_compositor_types::OutputId;

use crate::outputs::Outputs;
use crate::windows::{Windows, drawn_on};

#[derive(Debug, Default)]
pub(crate) struct Presence {
    entered: Vec<(SurfaceId, OutputId)>,
    visible: Vec<(SurfaceId, OutputId)>,
    covered: Vec<(SurfaceId, OutputId, i64)>,
    mains: HashMap<SurfaceId, OutputId>,
    chosen: Vec<(SurfaceId, OutputId)>,
    drawn: Vec<Drawn>,
    members: Vec<PlacedSurface>,
}

impl Presence {
    pub(crate) fn bound(&self, output: OutputId, resource: &WlOutput, surfaces: &mut Surfaces) {
        for (surface, _) in self
            .entered
            .iter()
            .filter(|(_, entered_output)| *entered_output == output)
        {
            surfaces.enter(*surface, resource);
        }
    }

    pub(crate) fn shown(&self, root: SurfaceId) -> bool {
        self.mains.contains_key(&root)
    }

    pub(crate) fn forget_output(&mut self, output: OutputId, surfaces: &mut Surfaces) {
        self.entered
            .retain(|(_, entered_output)| *entered_output != output);
        self.mains.retain(|root, main| {
            let remains_main = *main != output;
            if !remains_main {
                surfaces.set_main_output(*root, None);
            }
            remains_main
        });
    }
}

impl Presence {
    pub(crate) fn update(&mut self, outputs: &Outputs, windows: &Windows, surfaces: &mut Surfaces) {
        let presence = &mut *self;
        presence.visible.clear();
        presence.covered.clear();
        for output in outputs.enabled() {
            let Some(area) = output
                .area()
                .filter(|_| outputs.wayland().is_published(output.id()))
            else {
                continue;
            };
            drawn_on(windows, surfaces, output.id(), &mut presence.drawn);
            for item in &presence.drawn {
                let Drawn::Surface { root, location, .. } = *item else {
                    continue;
                };
                for placed in surfaces.tree(root) {
                    let Some(size) = surfaces
                        .get(placed.surface)
                        .and_then(|surface| surface.size())
                    else {
                        continue;
                    };
                    let rect = Rect::new(location + placed.location, size);
                    let Some(overlap) = area.intersection(rect) else {
                        continue;
                    };
                    presence.visible.push((placed.surface, output.id()));
                    presence.covered.push((
                        root,
                        output.id(),
                        i64::from(overlap.size.w) * i64::from(overlap.size.h),
                    ));
                }
            }
        }
        presence.visible.sort_unstable();
        presence.visible.dedup();
        choose(&mut presence.covered, &mut presence.chosen);
        self.enter_and_leave(outputs, surfaces);
        self.choose_mains(outputs, surfaces);
    }

    fn enter_and_leave(&mut self, outputs: &Outputs, surfaces: &mut Surfaces) {
        let presence = self;
        let wayland = outputs.wayland();
        let mut tell = |surface: SurfaceId, output: OutputId, entering: bool| {
            let Some(client) = surfaces.get(surface).map(|state| state.client()) else {
                return;
            };
            for resource in wayland.resources(output, client) {
                match entering {
                    true => surfaces.enter(surface, &resource),
                    false => surfaces.leave(surface, &resource),
                }
            }
        };
        for pair in &presence.visible {
            if presence.entered.binary_search(pair).is_err() {
                tell(pair.0, pair.1, true);
            }
        }
        for pair in &presence.entered {
            if presence.visible.binary_search(pair).is_err() {
                tell(pair.0, pair.1, false);
            }
        }
        std::mem::swap(&mut presence.entered, &mut presence.visible);
    }

    fn choose_mains(&mut self, outputs: &Outputs, surfaces: &mut Surfaces) {
        for index in 0..self.chosen.len() {
            let (root, output) = self.chosen[index];
            if self.mains.get(&root) != Some(&output) {
                surfaces.set_main_output(root, Some(output));
            }
            let Some(config) = outputs
                .get(output)
                .and_then(|output| output.enabled_config())
            else {
                continue;
            };
            let described = surfaces.output_image_description(output).cloned();
            surfaces.flatten_into(root, &mut self.members);
            for member in &self.members {
                surfaces.set_preferred_scale(member.surface, config.scale);
                surfaces.set_preferred_transform(member.surface, config.transform);
                if let Some(description) = &described {
                    surfaces.set_preferred_image_description(member.surface, description.clone());
                }
            }
        }
        let Presence { mains, chosen, .. } = self;
        for root in mains.keys() {
            if chosen
                .binary_search_by_key(root, |(entered_root, _)| *entered_root)
                .is_err()
            {
                surfaces.set_main_output(*root, None);
            }
        }
        mains.clear();
        mains.extend(chosen.iter().copied());
    }
}

fn choose(covered: &mut [(SurfaceId, OutputId, i64)], chosen: &mut Vec<(SurfaceId, OutputId)>) {
    covered.sort_unstable_by_key(|(root, output, _)| (*root, *output));
    chosen.clear();
    for by_root in covered.chunk_by(|a, b| a.0 == b.0) {
        let main = by_root
            .chunk_by(|a, b| a.1 == b.1)
            .map(|on| (on[0].1, on.iter().map(|(_, _, area)| area).sum::<i64>()))
            .max_by_key(|(_, area)| *area);
        if let Some((output, _)) = main {
            chosen.push((by_root[0].0, output));
        }
    }
}
