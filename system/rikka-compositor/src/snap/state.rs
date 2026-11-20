use std::time::{Duration, Instant};

use rikka_compositor_desktop::WindowId;
use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_compositor_snap::{Bar, Edged, Hit, Layout, Reveal, at_edge};
use rikka_compositor_types::OutputId;

use super::assist::Assist;
use super::motion::{Fade, Shape, Spread};
use crate::policy::SnapRules;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Where {
    pub(crate) output: OutputId,
    pub(crate) screen: Rect<i32, Logical>,
    pub(crate) area: Rect<i32, Logical>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Target {
    pub(crate) rect: Rect<i32, Logical>,
    pub(crate) maximize: bool,
}

#[derive(Debug)]
pub(crate) struct Aim {
    place: Where,
    bar: Option<Bar>,
    reveal: Reveal,
    hit: Option<Hit>,
    target: Option<Target>,
    layout: Option<Layout>,
}

impl Aim {
    pub(crate) fn new(place: Where) -> Aim {
        Aim {
            bar: Bar::measure(place.screen, place.area),
            place,
            reveal: Reveal::Hidden,
            hit: None,
            target: None,
            layout: None,
        }
    }

    pub(crate) fn aim_at(&mut self, at: Point<f64, Logical>, rules: SnapRules) -> bool {
        let previous = (self.reveal, self.hit, self.target);
        self.reveal = self
            .bar
            .as_ref()
            .filter(|_| rules.bar)
            .map_or(Reveal::Hidden, |bar| bar.revealed(at, self.reveal));
        self.hit = self
            .bar
            .as_ref()
            .filter(|_| self.reveal == Reveal::Open)
            .and_then(|bar| bar.hit(at));
        let choice = self.target_at(at);
        self.target = choice.map(|(target, _)| target);
        self.layout = choice.and_then(|(_, layout)| layout);
        previous != (self.reveal, self.hit, self.target)
    }

    fn target_at(&self, at: Point<f64, Logical>) -> Option<(Target, Option<Layout>)> {
        if let Some((bar, hit)) = self.bar.as_ref().zip(self.hit) {
            let layout = bar.tiles()[hit.tile].layout;
            return Some((
                Target {
                    rect: layout.zone(self.place.area, hit.zone),
                    maximize: false,
                },
                Some(layout),
            ));
        }
        match at_edge(self.place.screen, self.place.area, at)? {
            Edged::Maximize => Some((
                Target {
                    rect: self.place.area,
                    maximize: true,
                },
                None,
            )),
            Edged::Zone { layout, zone } => Some((
                Target {
                    rect: layout.zone(self.place.area, zone),
                    maximize: false,
                },
                Some(layout),
            )),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Drag {
    window: WindowId,
    aim: Aim,
    shown: Fade,
    spread: Spread,
    preview: Shape,
}

impl Drag {
    pub(crate) fn window(&self) -> WindowId {
        self.window
    }

    pub(crate) fn output(&self) -> OutputId {
        self.aim.place.output
    }

    pub(crate) fn area(&self) -> Rect<i32, Logical> {
        self.aim.place.area
    }

    pub(crate) fn target(&self) -> Option<Target> {
        self.aim.target
    }

    pub(crate) fn layout(&self) -> Option<Layout> {
        self.aim.layout
    }

    pub(crate) fn bar(&self) -> Option<&Bar> {
        self.aim.bar.as_ref().filter(|_| self.shown.value() > 0.0)
    }

    pub(crate) fn shown(&self) -> f32 {
        self.shown.value()
    }

    pub(crate) fn spread(&self) -> f32 {
        self.spread.value()
    }

    pub(crate) fn hit(&self) -> Option<Hit> {
        self.aim.hit
    }

    pub(crate) fn preview(&self) -> Option<(Rect<f64, Logical>, f32)> {
        self.preview.sample()
    }

    fn aim(&mut self, at: Point<f64, Logical>, rules: SnapRules) -> bool {
        let changed = self.aim.aim_at(at, rules);
        if self.aim.reveal == Reveal::Hidden {
            self.shown.leave();
        } else {
            self.shown.show();
        }
        self.spread.aim(self.aim.reveal == Reveal::Open);
        self.preview.aim(self.aim.target.map(|target| target.rect));
        changed
    }

    fn step(&mut self, elapsed: Duration) -> bool {
        let mut moving = self.shown.step(elapsed);
        moving |= self.spread.step(elapsed);
        moving |= self.preview.step(elapsed);
        moving
    }
}

#[derive(Debug, Default)]
pub(crate) struct Snapping {
    drag: Option<Drag>,
    assist: Option<Assist>,
    stepped: Option<Instant>,
}

impl Snapping {
    pub(crate) fn begin(&mut self, window: WindowId, place: Where) {
        self.drag = Some(Drag {
            window,
            aim: Aim::new(place),
            shown: Fade::hidden(),
            spread: Spread::shut(),
            preview: Shape::nowhere(),
        });
    }

    pub(crate) fn dragged(
        &mut self,
        place: Where,
        at: Point<f64, Logical>,
        rules: SnapRules,
    ) -> bool {
        let Some(drag) = &mut self.drag else {
            return false;
        };
        let moved = drag.aim.place != place;
        if moved {
            drag.aim = Aim::new(place);
        }
        drag.aim(at, rules) || moved
    }

    pub(crate) fn end(&mut self) {
        self.drag = None;
    }

    pub(crate) fn drag(&self) -> Option<&Drag> {
        self.drag.as_ref()
    }

    pub(crate) fn show_assist(&mut self, assist: Assist) {
        self.assist = Some(assist);
    }

    pub(crate) fn assist(&self) -> Option<&Assist> {
        self.assist.as_ref()
    }

    pub(crate) fn assist_mut(&mut self) -> Option<&mut Assist> {
        self.assist.as_mut()
    }

    pub(crate) fn dismiss_assist(
        &mut self,
        home: impl Fn(WindowId) -> Option<Rect<i32, Logical>>,
    ) -> bool {
        match &mut self.assist {
            Some(assist) if !assist.is_leaving() => {
                assist.dismiss(home);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn release_offer(&mut self, window: WindowId) {
        if let Some(assist) = &mut self.assist {
            assist.release(window);
        }
    }

    pub(crate) fn forget_assist(&mut self) {
        self.assist = None;
    }

    pub(crate) fn advance(&mut self, now: Instant) -> bool {
        let elapsed = self
            .stepped
            .map_or(Duration::ZERO, |stepped| now.duration_since(stepped));
        self.stepped = Some(now);
        let mut moving = self.drag.as_mut().is_some_and(|drag| drag.step(elapsed));
        if let Some(assist) = &mut self.assist {
            moving |= assist.step(elapsed);
            if assist.is_gone() {
                self.assist = None;
            }
        }
        moving
    }
}
