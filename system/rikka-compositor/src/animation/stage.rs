use std::time::Instant;

use rikka_compositor_caption::Button;
use rikka_compositor_desktop::{Desktop, Drawn, Target, WindowId};
use rikka_compositor_geometry::{Logical, Rect};
use rikka_compositor_types::OutputId;
use rikka_theme::Palette;

use super::animations::Animations;
use super::fades::Fade;
use super::lock_transition::{LockTransitionView, recedes};
use super::scrim::{Fader, Scrim};
use super::slide::SlidePosition;
use crate::policy::CaptionLook;
use crate::render::{Moved, Outline, Staging, View, Visual, WindowShape};
use crate::snap::Snapping;
use crate::windows::{ButtonLight, Windows};

const TRANSLUCENT_ALPHA: f32 = 0.4;

#[derive(Debug)]
pub(crate) struct StageContext<'a> {
    pub(crate) animations: &'a mut Animations,
    pub(crate) snap: &'a mut Snapping,
    pub(crate) windows: &'a Windows,
    pub(crate) palette: Palette,
    pub(crate) caption: CaptionLook,
    pub(crate) see_through: Option<WindowId>,
    pub(crate) scrims: &'a [Scrim],
}

impl StageContext<'_> {
    pub(crate) fn stage(
        &mut self,
        output: OutputId,
        drawn: &[Drawn],
        now: Instant,
    ) -> (Staging, bool) {
        let mut staging = Staging::default();
        let desktop = &self.windows.desktop;
        let Some(display) = desktop.output(output) else {
            return (staging, false);
        };
        let area = display.geometry();
        self.animations.advance(now);
        let mut running = self.snap.stage(
            output,
            View {
                area,
                scale: display.scale(),
            },
            self.palette,
            now,
            &mut staging,
            |window| desktop.window(window).map(|record| record.geometry()),
        );
        for item in drawn {
            let Drawn::Surface {
                target: Target::Window(window),
                ..
            } = *item
            else {
                continue;
            };
            let target = Target::Window(window);
            let Some(record) = desktop.window(window) else {
                continue;
            };
            if staging.moved.contains_key(&target) {
                continue;
            }
            let geometry = record.geometry();
            let stretched = desktop
                .policy(window)
                .and_then(|policy| policy.stretch)
                .and_then(|stretch| stretch.shown(geometry));
            let sampled = stretched.or_else(|| self.animations.sample(window, geometry, now));
            let shift = match self.animations.slid(window, output) {
                SlidePosition::Along(offset) => offset,
                SlidePosition::Still => 0.0,
                SlidePosition::Elsewhere => {
                    staging.hidden.push(target);
                    continue;
                }
            };
            running |= sampled.is_some() || shift != 0.0;
            let see_through = self.see_through == Some(window);
            let shown = faded(shifted(sampled, geometry, shift), geometry, see_through);
            if let Some(shown) = shown {
                staging.moved.insert(target, Moved { geometry, shown });
            }
        }
        for (leaving, window) in self.animations.exiting(now) {
            running |= leaving.geometry.overlaps(area);
            match window {
                Some(_) => {
                    staging.exiting.insert(leaving.snapshot, leaving.shown);
                }
                None => staging.leaving.push(leaving),
            }
        }
        staging.leaving.extend(self.animations.leaving(output));
        running |= self.animations.sliding(output);
        running |= self.animations.lock_transition.stage(
            &LockTransitionView {
                output,
                area,
                drawn,
                now,
            },
            &mut staging,
            |target| receding(desktop, target),
        );
        running |= self.stage_shapes(drawn, &mut staging);
        running |= self.stage_outlines(drawn, &mut staging);
        running |= self.stage_captions(drawn, &mut staging);
        running |= self.stage_scrims(drawn, &mut staging);
        for item in drawn {
            let Drawn::Surface {
                target: target @ Target::Popup(_),
                ..
            } = *item
            else {
                continue;
            };
            let root = desktop.root_of(target);
            if let Some(moved) = staging.moved.get(&root).copied() {
                staging.moved.insert(target, moved);
            }
            if staging.lifted.contains(&root) {
                staging.lifted.push(target);
            }
            if staging.hidden.contains(&root) {
                staging.hidden.push(target);
            }
        }
        (staging, running)
    }

    fn stage_shapes(&mut self, drawn: &[Drawn], staging: &mut Staging) -> bool {
        let mut running = false;
        for item in drawn {
            let Drawn::Surface {
                target: Target::Window(window),
                ..
            } = *item
            else {
                continue;
            };
            let Some(shape) = self.windows.shape(window) else {
                continue;
            };
            let rounding = self.animations.round(window, shape.rounding);
            running |= !rounding.settled;
            staging.shapes.insert(
                window,
                WindowShape {
                    rounding: rounding.value,
                    ..shape
                },
            );
        }
        running
    }

    fn stage_captions(&mut self, drawn: &[Drawn], staging: &mut Staging) -> bool {
        let windows = self.windows;
        let pointer = &windows.captions;
        let buttons: Vec<Fade<(WindowId, Button)>> =
            self.animations.caption_buttons(pointer.lit()).collect();
        let focused = windows
            .recency
            .current()
            .filter(|window| windows.is_decorated(*window));
        let focus: Vec<Fade<WindowId>> = self.animations.caption_focus(focused).collect();
        let mut running = false;
        for item in drawn {
            let Drawn::Surface {
                target: Target::Window(window),
                ..
            } = *item
            else {
                continue;
            };
            let focus = focus.iter().find(|fade| fade.key == window);
            let lit = buttons.iter().filter(|fade| fade.key.0 == window);
            running |= focus.is_some_and(|fade| !fade.settled);
            running |= lit.clone().any(|fade| !fade.settled);
            let light = |button: Button| ButtonLight {
                amount: lit
                    .clone()
                    .find(|fade| fade.key.1 == button)
                    .map_or(0.0, |fade| fade.opacity),
                pressed: pointer.is_held(window, button),
            };
            let focused = focus.map_or(0.0, |fade| fade.opacity);
            if let Some(paint) = windows.caption_paint(window, self.caption, focused, light) {
                staging.captions.insert(window, paint);
            }
        }
        running
    }

    fn stage_outlines(&mut self, drawn: &[Drawn], staging: &mut Staging) -> bool {
        let selection = self.palette.selection;
        let mut running = false;
        for fade in self.animations.outline(self.windows.outlined()) {
            if !is_drawn(drawn, Target::Window(fade.key)) {
                continue;
            }
            running |= !fade.settled;
            staging.outlines.push(Outline {
                window: fade.key,
                colour: selection.scale_alpha(fade.opacity),
            });
        }
        running
    }

    fn stage_scrims(&mut self, drawn: &[Drawn], staging: &mut Staging) -> bool {
        let scrims = self.scrims;
        let mut running = false;
        for fade in self.animations.scrims(scrims) {
            let layer = Target::Layer(fade.key);
            if !is_drawn(drawn, layer) {
                continue;
            }
            running |= !fade.settled;
            staging.scrims.insert(layer, fade.opacity);
        }
        for scrim in scrims {
            let layer = Target::Layer(scrim.layer);
            if scrim.fader == Fader::Client && scrim.frosted && is_drawn(drawn, layer) {
                staging.scrims.insert(layer, 1.0);
            }
        }
        running
    }
}

fn is_drawn(drawn: &[Drawn], wanted: Target) -> bool {
    drawn
        .iter()
        .any(|item| matches!(*item, Drawn::Surface { target, .. } if target == wanted))
}

fn shifted(shown: Option<Visual>, geometry: Rect<i32, Logical>, shift: f64) -> Option<Visual> {
    if shift == 0.0 {
        return shown;
    }
    Some(
        shown
            .unwrap_or_else(|| Visual::opaque(geometry))
            .shifted(shift),
    )
}

fn faded(shown: Option<Visual>, geometry: Rect<i32, Logical>, see_through: bool) -> Option<Visual> {
    if !see_through {
        return shown;
    }
    Some(
        shown
            .unwrap_or_else(|| Visual::opaque(geometry))
            .faded(TRANSLUCENT_ALPHA),
    )
}

fn receding<P>(desktop: &Desktop<P>, target: Target) -> Option<(Rect<i32, Logical>, bool)> {
    let geometry = match target {
        Target::Window(window) => desktop.window(window)?.geometry(),
        Target::Layer(layer) => desktop.layer(layer)?.geometry(),
        Target::Popup(_) | Target::Lock(_) => return None,
    };
    Some((geometry, recedes(desktop, target)))
}
