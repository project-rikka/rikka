use std::collections::HashMap;
use std::time::Instant;

use rikka_animation::{Curve, DurationToken, Easing};
use rikka_compositor_desktop::{Desktop, Drawn, SnapshotId, Target};
use rikka_compositor_geometry::{Logical, Point, Rect, Size};
use rikka_compositor_protocols::zwlr_layer_shell_v1::Layer as Level;
use rikka_compositor_types::OutputId;

use super::track::Track;
use crate::render::{Leaving, Moved, Staging, Visual};

const LOWER: Curve = Curve::Tween {
    easing: Easing::EmphasizedDecel,
    duration: DurationToken::Medium2,
};

const RAISE: Curve = Curve::Tween {
    easing: Easing::EmphasizedDecel,
    duration: DurationToken::Long2,
};

const LOWERED_SCALE: f64 = 0.97;

const UNLOCK_START_SCALE: f64 = 0.94;

const LOCK_END_SCALE: f64 = 1.06;

const DESKTOP_FADE_ALPHA: f32 = 0.6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Lowered {
    pub(crate) snapshot: SnapshotId,
    pub(crate) rect: Rect<i32, Logical>,
    pub(crate) recedes: bool,
}

#[derive(Debug)]
struct Lowering {
    desktop: Vec<Lowered>,
    fades: HashMap<OutputId, (Track, Instant)>,
}

#[derive(Debug)]
struct Raising {
    lock: Option<(SnapshotId, Rect<i32, Logical>)>,
    track: Track,
    stepped: Instant,
}

#[derive(Debug, Default)]
pub(crate) struct LockTransition {
    lowering: Option<Lowering>,
    raising: HashMap<OutputId, Raising>,
    released: Vec<SnapshotId>,
}

pub(crate) struct LockTransitionView<'a> {
    pub(crate) output: OutputId,
    pub(crate) area: Rect<i32, Logical>,
    pub(crate) drawn: &'a [Drawn],
    pub(crate) now: Instant,
}

impl LockTransition {
    pub(crate) fn lower(&mut self, desktop: Vec<Lowered>) {
        self.drop_all();
        self.lowering = Some(Lowering {
            desktop,
            fades: HashMap::new(),
        });
    }

    pub(crate) fn raise(
        &mut self,
        output: OutputId,
        lock: Option<(SnapshotId, Rect<i32, Logical>)>,
    ) {
        self.drop_lowering();
        let mut track = Track::new(0.0, RAISE);
        track.aim(1.0, RAISE);
        if let Some(old) = self.raising.insert(
            output,
            Raising {
                lock,
                track,
                stepped: Instant::now(),
            },
        ) {
            self.released.extend(old.lock.map(|(snapshot, _)| snapshot));
        }
    }

    pub(crate) fn cancel_lowering(&mut self) {
        self.drop_lowering();
    }

    pub(crate) fn drop_all(&mut self) {
        self.drop_lowering();
        let raising = std::mem::take(&mut self.raising);
        self.released.extend(
            raising
                .into_values()
                .filter_map(|raising| raising.lock.map(|(snapshot, _)| snapshot)),
        );
    }

    fn drop_lowering(&mut self) {
        if let Some(lowering) = self.lowering.take() {
            self.released
                .extend(lowering.desktop.into_iter().map(|frozen| frozen.snapshot));
        }
    }

    pub(crate) fn covers(&self, output: OutputId) -> bool {
        let Some(lowering) = &self.lowering else {
            return true;
        };
        lowering
            .fades
            .get(&output)
            .is_some_and(|(track, _)| track.is_resting())
    }

    pub(crate) fn take_released(&mut self) -> Vec<SnapshotId> {
        std::mem::take(&mut self.released)
    }

    pub(crate) fn advance(&mut self, now: Instant) {
        if let Some(lowering) = &mut self.lowering {
            for (track, stepped) in lowering.fades.values_mut() {
                track.step(now.saturating_duration_since(*stepped));
                *stepped = (*stepped).max(now);
            }
        }
        let finished: Vec<OutputId> = self
            .raising
            .iter_mut()
            .filter_map(|(output, raising)| {
                raising
                    .track
                    .step(now.saturating_duration_since(raising.stepped));
                raising.stepped = raising.stepped.max(now);
                raising.track.is_resting().then_some(*output)
            })
            .collect();
        for output in finished {
            if let Some(raising) = self.raising.remove(&output) {
                self.released
                    .extend(raising.lock.map(|(snapshot, _)| snapshot));
            }
        }
    }

    pub(crate) fn stage(
        &mut self,
        view: &LockTransitionView<'_>,
        staging: &mut Staging,
        geometry_of: impl Fn(Target) -> Option<(Rect<i32, Logical>, bool)>,
    ) -> bool {
        let lowered = self.stage_lowering(view, staging);
        let raised = self.stage_raising(view, staging, geometry_of);
        lowered || raised
    }

    fn stage_lowering(&mut self, view: &LockTransitionView<'_>, staging: &mut Staging) -> bool {
        let Some(lowering) = &mut self.lowering else {
            return false;
        };
        let guard = view.drawn.iter().find_map(|drawn| match *drawn {
            Drawn::Surface {
                target: target @ Target::Lock(_),
                ..
            } => Some(target),
            _ => None,
        });
        let fade = guard.map(|_| {
            &lowering
                .fades
                .entry(view.output)
                .or_insert_with(|| {
                    let mut track = Track::new(0.0, LOWER);
                    track.aim(1.0, LOWER);
                    (track, view.now)
                })
                .0
        });
        let progress = fade.map_or(0.0, |track| f64::from(track.value()));
        let centre = centre_of(view.area);
        for frozen in &lowering.desktop {
            if !frozen.rect.overlaps(view.area) {
                continue;
            }
            let scale = match frozen.recedes {
                true => 1.0 + (LOWERED_SCALE - 1.0) * progress,
                false => 1.0,
            };
            staging.leaving.push(Leaving {
                snapshot: frozen.snapshot,
                geometry: frozen.rect,
                shown: Visual {
                    rect: zoomed(frozen.rect.to_f64(), centre, scale),
                    alpha: 1.0,
                },
            });
        }
        let Some((target, track)) = guard.zip(fade) else {
            return false;
        };
        staging.lifted.push(target);
        staging.moved.insert(
            target,
            Moved {
                geometry: view.area,
                shown: Visual {
                    rect: view.area.to_f64(),
                    alpha: track.value(),
                },
            },
        );
        !track.is_resting()
    }

    fn stage_raising(
        &mut self,
        view: &LockTransitionView<'_>,
        staging: &mut Staging,
        geometry_of: impl Fn(Target) -> Option<(Rect<i32, Logical>, bool)>,
    ) -> bool {
        let Some(raising) = self.raising.get(&view.output) else {
            return false;
        };
        let progress = raising.track.value();
        let centre = centre_of(view.area);
        let scale = UNLOCK_START_SCALE + (1.0 - UNLOCK_START_SCALE) * f64::from(progress);
        let alpha = (progress / DESKTOP_FADE_ALPHA).min(1.0);
        for drawn in view.drawn {
            let Drawn::Surface { target, .. } = *drawn else {
                continue;
            };
            if matches!(target, Target::Popup(_)) {
                continue;
            }
            let Some((geometry, recedes)) = geometry_of(target) else {
                continue;
            };
            if !recedes {
                continue;
            }
            let base = staging
                .moved
                .get(&target)
                .map_or_else(|| Visual::opaque(geometry), |moved| moved.shown);
            staging.moved.insert(
                target,
                Moved {
                    geometry,
                    shown: Visual {
                        rect: zoomed(base.rect, centre, scale),
                        alpha: base.alpha * alpha,
                    },
                },
            );
        }
        if let Some((snapshot, rect)) = raising.lock {
            staging.leaving.push(Leaving {
                snapshot,
                geometry: rect,
                shown: Visual {
                    rect: zoomed(
                        rect.to_f64(),
                        centre,
                        1.0 + (LOCK_END_SCALE - 1.0) * f64::from(progress),
                    ),
                    alpha: 1.0 - progress,
                },
            });
        }
        true
    }
}

pub(crate) fn recedes<P>(desktop: &Desktop<P>, target: Target) -> bool {
    match target {
        Target::Layer(layer) => desktop
            .layer(layer)
            .is_some_and(|record| !matches!(record.level(), Level::Background | Level::Bottom)),
        Target::Window(_) | Target::Popup(_) => true,
        Target::Lock(_) => false,
    }
}

fn centre_of(area: Rect<i32, Logical>) -> Point<f64, Logical> {
    let area = area.to_f64();
    Point::new(
        area.loc.x + area.size.w / 2.0,
        area.loc.y + area.size.h / 2.0,
    )
}

fn zoomed(rect: Rect<f64, Logical>, centre: Point<f64, Logical>, scale: f64) -> Rect<f64, Logical> {
    Rect::new(
        Point::new(
            centre.x + (rect.loc.x - centre.x) * scale,
            centre.y + (rect.loc.y - centre.y) * scale,
        ),
        Size::new(rect.size.w * scale, rect.size.h * scale),
    )
}
