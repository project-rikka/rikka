use std::sync::Arc;
use std::time::Duration;

use rikka_compositor_desktop::WindowId;
use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_compositor_snap::{Layout, Slot, fitted, shape_of};
use rikka_compositor_types::OutputId;
use rikka_types::RoundedRect;

use crate::animation::Flight;
use crate::glass::{Frost, frost};
use crate::render::Visual;

use super::motion::{Fade, TRAVEL};

#[derive(Debug)]
pub(crate) struct Card {
    pub(crate) window: WindowId,
    pub(crate) title: Arc<str>,
    pub(crate) slot: Slot,
    home: Rect<i32, Logical>,
    flight: Flight,
}

impl Card {
    pub(crate) fn new(
        window: WindowId,
        title: Arc<str>,
        home: Rect<i32, Logical>,
        slot: Slot,
        from: Option<Rect<f64, Logical>>,
    ) -> Card {
        let mut flight = Flight::new(from.unwrap_or_else(|| home.to_f64()), TRAVEL);
        flight.aim(snapped_area(slot, home));
        Card {
            window,
            title,
            slot,
            home,
            flight,
        }
    }

    pub(crate) fn rect(&self) -> Rect<f64, Logical> {
        self.flight.rect()
    }

    fn return_home(&mut self, home: Option<Rect<i32, Logical>>) {
        self.home = home.unwrap_or(self.home);
        self.flight.aim(self.home.to_f64());
    }
}

fn snapped_area(slot: Slot, window: Rect<i32, Logical>) -> Rect<f64, Logical> {
    fitted(slot.picture, shape_of(window.size))
}

#[derive(Debug)]
pub(crate) struct Assist {
    output: OutputId,
    layout: Layout,
    empty: Vec<Rect<i32, Logical>>,
    frost: Arc<Frost>,
    target: Rect<i32, Logical>,
    cards: Vec<Card>,
    hovered: Option<usize>,
    fade: Fade,
}

impl Assist {
    pub(crate) fn new(
        output: OutputId,
        layout: Layout,
        empty: Vec<Rect<i32, Logical>>,
        target: Rect<i32, Logical>,
        cards: Vec<Card>,
    ) -> Self {
        let zones: Vec<RoundedRect> = empty
            .iter()
            .map(|zone| RoundedRect::rect((*zone).into()))
            .collect();
        Self {
            output,
            layout,
            frost: Arc::new(frost(&zones)),
            empty,
            target,
            cards,
            hovered: None,
            fade: Fade::arriving(),
        }
    }

    pub(crate) fn output(&self) -> OutputId {
        self.output
    }

    pub(crate) fn layout(&self) -> Layout {
        self.layout
    }

    pub(crate) fn frost(&self) -> &Arc<Frost> {
        &self.frost
    }

    pub(crate) fn cards(&self) -> &[Card] {
        &self.cards
    }

    pub(crate) fn contains_window(&self, window: WindowId) -> bool {
        self.cards.iter().any(|card| card.window == window)
    }

    pub(crate) fn release(&mut self, window: WindowId) {
        self.cards.retain(|card| card.window != window);
        self.hovered = None;
    }

    pub(crate) fn card_rect(&self, window: WindowId) -> Option<Rect<f64, Logical>> {
        self.cards
            .iter()
            .find(|card| card.window == window)
            .map(Card::rect)
    }

    pub(crate) fn visual_for(&self, window: WindowId) -> Option<Visual> {
        self.card_rect(window)
            .map(|rect| Visual { rect, alpha: 1.0 })
    }

    pub(crate) fn hovered(&self) -> Option<usize> {
        self.hovered
    }

    pub(crate) fn alpha(&self) -> f32 {
        self.fade.value()
    }

    pub(crate) fn dismiss(&mut self, home: impl Fn(WindowId) -> Option<Rect<i32, Logical>>) {
        self.fade.leave();
        self.hovered = None;
        for card in &mut self.cards {
            card.return_home(home(card.window));
        }
    }

    pub(crate) fn is_leaving(&self) -> bool {
        self.fade.is_leaving()
    }

    pub(crate) fn is_gone(&self) -> bool {
        self.fade.is_gone() && self.cards.iter().all(|card| card.flight.is_resting())
    }

    pub(crate) fn covers(&self, at: Point<f64, Logical>) -> bool {
        !self.is_leaving() && self.empty.iter().any(|zone| zone.to_f64().contains(at))
    }

    pub(crate) fn hover(&mut self, at: Point<f64, Logical>) -> bool {
        let over = self.card_at(at);
        let changed = over != self.hovered;
        self.hovered = over;
        changed
    }

    pub(crate) fn card_at_point(
        &self,
        at: Point<f64, Logical>,
    ) -> Option<(WindowId, Rect<i32, Logical>)> {
        let card = self.card_at(at)?;
        let card = self.cards.get(card)?;
        Some((card.window, self.target))
    }

    pub(crate) fn step(&mut self, elapsed: Duration) -> bool {
        let mut moving = self.fade.step(elapsed);
        for card in &mut self.cards {
            moving |= card.flight.step(elapsed);
        }
        moving
    }

    fn card_at(&self, at: Point<f64, Logical>) -> Option<usize> {
        if self.is_leaving() {
            return None;
        }
        self.cards
            .iter()
            .position(|card| card.slot.card.to_f64().contains(at))
    }
}
