use std::time::Instant;

use rikka_theme::Theme;
use rikka_types::{Key, Modifiers, Point, Rect, Size};
use rikka_ui::{Event, EventCtx, PaintCtx, side_panel};

use crate::lists::collection::KeyboardWalk;
use crate::navigation::menu::MenuItem;
use crate::navigation::menu::panel::{Listing, PADDING, Panel};
use crate::navigation::menu::typeahead::Typeahead;

const OVERLAP: f32 = -4.0;

pub(crate) type Laid<'a> = Vec<(Panel<'a>, Rect)>;

#[derive(Debug)]
pub(crate) struct Cascade {
    levels: Vec<Level>,
    typeahead: Typeahead,
    sliding: Option<(usize, usize)>,
}

#[derive(Debug)]
struct Level {
    opened_by: usize,
    listing: Listing,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Press {
    Away,
    Inert,
    Opened,
    Leaf {
        level: usize,
        row: usize,
    },
    Slide {
        level: usize,
        row: usize,
        value: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Keyed {
    Ignored,
    Kept,
    Changed,
    Choose {
        level: usize,
        row: usize,
    },
    Slid {
        level: usize,
        row: usize,
        value: f32,
    },
    Close,
    Aside {
        forward: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Pointed {
    Kept,
    Changed,
    Pressed(Press),
    Slid {
        level: usize,
        row: usize,
        value: f32,
    },
}

impl Keyed {
    fn changed(changed: bool) -> Keyed {
        match changed {
            true => Keyed::Changed,
            false => Keyed::Kept,
        }
    }
}

impl Pointed {
    fn changed(changed: bool) -> Pointed {
        match changed {
            true => Pointed::Changed,
            false => Pointed::Kept,
        }
    }
}

impl Default for Cascade {
    fn default() -> Self {
        Self {
            levels: vec![Level {
                opened_by: 0,
                listing: Listing::default(),
            }],
            typeahead: Typeahead::default(),
            sliding: None,
        }
    }
}

impl Cascade {
    pub(crate) fn root(&self) -> &Listing {
        &self.levels[0].listing
    }

    pub(crate) fn root_mut(&mut self) -> &mut Listing {
        &mut self.levels[0].listing
    }

    pub(crate) fn branched(&self) -> bool {
        self.levels.len() > 1
    }

    pub(crate) fn collapse(&mut self) {
        self.levels.truncate(1);
        self.sliding = None;
    }

    pub(crate) fn is_sliding(&self) -> bool {
        self.sliding.is_some()
    }

    pub(crate) fn path(&self, level: usize) -> impl Iterator<Item = usize> + '_ {
        self.levels[1..=level].iter().map(|level| level.opened_by)
    }

    pub(crate) fn relist(&mut self) {
        self.collapse();
        self.root_mut().relist();
    }

    pub(crate) fn lay<'a>(
        &mut self,
        root: Panel<'a>,
        bounds: Rect,
        theme: &Theme,
        viewport: Size,
    ) -> Laid<'a> {
        let mut laid = vec![(root, bounds)];
        for level in 1..self.levels.len() {
            let (parent, beside) = laid[level - 1];
            let opened_by = self.levels[level].opened_by;
            let Some(items) = parent.branch(opened_by) else {
                break;
            };
            let row = parent.row_rect(beside, self.levels[level - 1].listing.scrolled(), opened_by);
            let panel = Panel::new(items, parent.size);
            let rect = side_panel(
                beside,
                Point::new(OVERLAP, row.y() - PADDING),
                panel.size_beside(theme),
                viewport,
            );
            laid.push((panel, rect));
        }
        self.levels.truncate(laid.len());
        laid
    }

    pub(crate) fn key(&mut self, laid: &Laid<'_>, key: Key, modifiers: Modifiers) -> Keyed {
        let now = Instant::now();
        let typing = self.typeahead.is_typing(now);
        if let Some((level, row)) = self.chosen()
            && let Some(value) = laid[level]
                .0
                .items
                .get(row)
                .and_then(|item| item.slider.as_ref())
                .and_then(|slider| slider.keyed(key))
        {
            return Keyed::Slid { level, row, value };
        }
        match key {
            Key::ArrowUp | Key::ArrowDown => {
                self.step(laid, key == Key::ArrowDown);
                Keyed::Changed
            }
            Key::Home | Key::End | Key::PageUp | Key::PageDown => {
                Keyed::changed(self.walk(laid, key))
            }
            Key::ArrowRight if self.enter(laid) => Keyed::Changed,
            Key::ArrowRight => Keyed::Aside { forward: true },
            Key::ArrowLeft if self.close_active() => Keyed::Changed,
            Key::ArrowLeft => Keyed::Aside { forward: false },
            Key::Space if typing => Keyed::changed(self.typed(laid, ' ', now)),
            Key::Enter | Key::Space if self.enter(laid) => Keyed::Changed,
            Key::Enter | Key::Space => match self.chosen() {
                Some((level, row)) => Keyed::Choose { level, row },
                None => Keyed::Kept,
            },
            Key::Escape if self.close_deepest() => Keyed::Changed,
            Key::Escape => Keyed::Close,
            Key::Char(letter) if !(modifiers.control || modifiers.alt || modifiers.logo) => {
                Keyed::changed(self.typed(laid, letter, now))
            }
            _ => Keyed::Ignored,
        }
    }

    pub(crate) fn pointer(&mut self, laid: &Laid<'_>, event: &Event) -> Option<Pointed> {
        if let Some((level, row)) = self.sliding {
            return match *event {
                Event::PointerMoved(at) => {
                    let (panel, rect) = laid[level];
                    let scrolled = self.levels[level].listing.scrolled();
                    let value = panel.slider_value(rect, scrolled, row, at.x)?;
                    Some(Pointed::Slid { level, row, value })
                }
                Event::PointerReleased { .. } | Event::PointerCancelled => {
                    self.sliding = None;
                    Some(Pointed::Changed)
                }
                _ => Some(Pointed::Kept),
            };
        }
        Some(match *event {
            Event::PointerMoved(at) => Pointed::changed(self.point(laid, at, Instant::now())),
            Event::PointerExited => Pointed::changed(self.leave(laid)),
            Event::Scroll {
                delta, position, ..
            } => Pointed::changed(self.scroll(laid, position, delta.offset().y)),
            Event::PointerPressed { position, .. } => {
                let press = self.press(laid, position);
                if let Press::Slide { level, row, .. } = press {
                    self.sliding = Some((level, row));
                }
                Pointed::Pressed(press)
            }
            _ => return None,
        })
    }

    fn point(&mut self, laid: &Laid<'_>, at: Point, now: Instant) -> bool {
        let Some(level) = row_at(laid, at) else {
            return self.leave(laid);
        };
        let (panel, rect) = laid[level];
        let listing = &self.levels[level].listing;
        let next = panel.command_at(rect, listing.scrolled(), at.y);
        if next != listing.highlighted()
            && let Some(toward) = aimed_at(laid, level, listing.highlighted())
            && listing.holds_aim(toward, at, now)
        {
            return false;
        }
        let changed = self.choose(laid, level, next);
        if panel.opens(next) {
            self.levels[level].listing.aim_from(at, now);
        }
        changed
    }

    fn leave(&mut self, laid: &Laid<'_>) -> bool {
        for level in &mut self.levels {
            level.listing.drop_aim();
        }
        let deepest = self.levels.len() - 1;
        let listing = &mut self.levels[deepest].listing;
        let holding = laid
            .get(deepest)
            .is_some_and(|(panel, _)| panel.opens(listing.highlighted()));
        !holding && listing.highlight(None)
    }

    pub(crate) fn press(&mut self, laid: &Laid<'_>, at: Point) -> Press {
        let Some(level) = row_at(laid, at) else {
            return Press::Away;
        };
        let (panel, rect) = laid[level];
        let scrolled = self.levels[level].listing.scrolled();
        let Some(row) = panel.command_at(rect, scrolled, at.y) else {
            return Press::Inert;
        };
        if panel.is_slider(row) {
            self.choose(laid, level, Some(row));
            return match panel.slider_hit(rect, scrolled, row, at.x) {
                Some(value) => Press::Slide { level, row, value },
                None => Press::Inert,
            };
        }
        if panel.branch(row).is_none() {
            return Press::Leaf { level, row };
        }
        self.choose(laid, level, Some(row));
        Press::Opened
    }

    fn scroll(&mut self, laid: &Laid<'_>, at: Point, travel: f32) -> bool {
        let level = row_at(laid, at).unwrap_or(0);
        let (panel, rect) = laid[level];
        let listing = &mut self.levels[level].listing;
        if !panel.scroll_by(listing, rect, travel) {
            return false;
        }
        if panel.opens(listing.highlighted()) {
            listing.highlight(None);
            self.levels.truncate(level + 1);
        }
        true
    }

    pub(crate) fn chosen(&self) -> Option<(usize, usize)> {
        let level = self.active();
        Some((level, self.levels[level].listing.highlighted()?))
    }

    pub(crate) fn step(&mut self, laid: &Laid<'_>, forward: bool) {
        let level = self.active();
        let (panel, rect) = laid[level];
        self.levels.truncate(level + 1);
        let listing = &mut self.levels[level].listing;
        let next = panel.step(listing.highlighted(), forward);
        listing.highlight(next);
        if let Some(row) = next {
            panel.reveal(listing, rect, row);
        }
    }

    fn walk(&mut self, laid: &Laid<'_>, key: Key) -> bool {
        let level = self.active();
        let (panel, rect) = laid[level];
        let from = self.levels[level].listing.highlighted();
        let walk = KeyboardWalk::column(panel.items.len(), panel.page_rows(rect));
        let Some(Some(row)) = walk.to(key, from, |row| panel.items[row].is_command()) else {
            return false;
        };
        self.land(laid, level, row);
        true
    }

    fn typed(&mut self, laid: &Laid<'_>, letter: char, now: Instant) -> bool {
        self.typeahead.push(letter, now);
        let level = self.active();
        let from = self.levels[level].listing.highlighted();
        let Some(row) = self.typeahead.find(laid[level].0.items, from) else {
            return false;
        };
        self.land(laid, level, row);
        true
    }

    pub(crate) fn preselect(&mut self, laid: &Laid<'_>, row: usize) {
        let (panel, rect) = laid[0];
        if !panel.items.get(row).is_some_and(MenuItem::is_command) {
            return;
        }
        self.collapse();
        let listing = self.root_mut();
        listing.highlight(Some(row));
        panel.reveal_at_once(listing, rect, row);
    }

    pub(crate) fn highlight(&mut self, laid: &Laid<'_>, row: usize) {
        self.collapse();
        self.land(laid, 0, row);
    }

    fn land(&mut self, laid: &Laid<'_>, level: usize, row: usize) {
        let (panel, rect) = laid[level];
        self.levels.truncate(level + 1);
        let listing = &mut self.levels[level].listing;
        listing.highlight(Some(row));
        panel.reveal(listing, rect, row);
    }

    fn enter(&mut self, laid: &Laid<'_>) -> bool {
        let Some((level, row)) = self.chosen() else {
            return false;
        };
        let (panel, _) = laid[level];
        let Some(items) = panel.branch(row) else {
            return false;
        };
        self.levels.truncate(level + 1);
        self.levels.push(Level {
            opened_by: row,
            listing: Listing::unseen(),
        });
        let first = Panel::new(items, panel.size).step(None, true);
        self.levels[level + 1].listing.highlight(first);
        true
    }

    fn close_active(&mut self) -> bool {
        let level = self.active();
        if level == 0 {
            return false;
        }
        self.levels.truncate(level);
        true
    }

    fn close_deepest(&mut self) -> bool {
        if !self.branched() {
            return false;
        }
        self.levels.pop();
        true
    }

    pub(crate) fn paint_branches(&mut self, ctx: &mut PaintCtx<'_>, laid: &Laid<'_>, shown: f32) {
        for (level, (panel, rect)) in laid.iter().enumerate().skip(1) {
            let (parent, beside) = laid[level - 1];
            let opener = parent
                .row_rect(
                    beside,
                    self.levels[level - 1].listing.scrolled(),
                    self.levels[level].opened_by,
                )
                .translate(beside.origin);
            let pivot = rect.nearest(opener.center());
            let listing = &mut self.levels[level].listing;
            ctx.motion_scope(("menu.branch", level), |ctx| {
                let own = listing.reveal(ctx, true);
                panel.paint(ctx, listing, *rect, own * shown, pivot, |_, _| {});
            });
        }
    }

    fn active(&self) -> usize {
        (0..self.levels.len())
            .rev()
            .find(|level| self.levels[*level].listing.highlighted().is_some())
            .unwrap_or(0)
    }

    fn choose(&mut self, laid: &Laid<'_>, level: usize, next: Option<usize>) -> bool {
        if !self.levels[level].listing.highlight(next) {
            return false;
        }
        self.levels.truncate(level + 1);
        if let Some(row) = next
            && laid[level].0.branch(row).is_some()
        {
            self.levels.push(Level {
                opened_by: row,
                listing: Listing::unseen(),
            });
        }
        true
    }
}

pub(crate) fn leaf<'a>(laid: &Laid<'a>, level: usize, row: usize) -> Option<&'a MenuItem> {
    laid.get(level)?.0.items.get(row)
}

pub(crate) fn slide(
    laid: &Laid<'_>,
    (level, row): (usize, usize),
    value: f32,
    ctx: &mut EventCtx<'_>,
) {
    if let Some(item) = leaf(laid, level, row) {
        item.slid(value, ctx);
    }
}

fn row_at(laid: &Laid<'_>, at: Point) -> Option<usize> {
    (0..laid.len())
        .rev()
        .find(|level| laid[*level].1.contains(at))
}

fn aimed_at(laid: &Laid<'_>, level: usize, open_branch: Option<usize>) -> Option<Rect> {
    if let Some((_, beside)) = laid.get(level + 1) {
        return Some(*beside);
    }
    let (panel, rect) = laid[level];
    panel
        .opens(open_branch)
        .then(|| Rect::new(rect.max_x(), rect.y(), 0.0, rect.height()))
}
