use rikka_compositor_geometry::{Logical, Point, Rect, Size};

use crate::layout::{Layout, templates};

const TILE_HEIGHT: i32 = 84;
const TILE_WIDTH_LEAST: i32 = 104;
const TILE_WIDTH_MOST: i32 = 168;
const TILE_GAP: i32 = 10;
const PADDING: i32 = 12;
const ZONE_GAP: i32 = 4;
const TILE_INSET: i32 = 6;
const MARGIN: i32 = 24;

const ABOVE: i32 = 8;
const PEEK: i32 = PADDING;

const REVEAL_BAND: f64 = 12.0;
const REVEAL_SPREAD: f64 = 48.0;
const GRACE: f64 = 40.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reveal {
    Hidden,
    Peek,
    Open,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub tile: usize,
    pub zone: usize,
}

#[derive(Clone, Debug)]
pub struct Tile {
    pub layout: Layout,
    pub rect: Rect<i32, Logical>,
}

impl Tile {
    pub fn zones(&self) -> impl Iterator<Item = Rect<i32, Logical>> {
        self.layout.zones(inset_tile(self.rect)).map(inset)
    }
}

#[derive(Clone, Debug)]
pub struct Bar {
    rect: Rect<i32, Logical>,
    tiles: Vec<Tile>,
}

impl Bar {
    pub fn measure(screen: Rect<i32, Logical>, area: Rect<i32, Logical>) -> Option<Bar> {
        if area.size.w <= 0 || area.size.h <= 0 {
            return None;
        }
        let layouts = templates(area.size);
        let count = i32::try_from(layouts.len()).unwrap_or(0);
        let shape =
            i32::try_from(i64::from(TILE_HEIGHT) * i64::from(area.size.w) / i64::from(area.size.h))
                .unwrap_or(TILE_WIDTH_MOST);
        let tile_width = shape.clamp(TILE_WIDTH_LEAST, TILE_WIDTH_MOST);
        let width = count * tile_width + (count - 1) * TILE_GAP + 2 * PADDING;
        let height = TILE_HEIGHT + 2 * PADDING;
        if width > screen.size.w - 2 * MARGIN {
            return None;
        }

        let rect = Rect::new(
            Point::new(
                screen.loc.x + (screen.size.w - width) / 2,
                area.loc.y + ABOVE,
            ),
            Size::new(width, height),
        );
        let tiles = layouts
            .iter()
            .enumerate()
            .map(|(index, layout)| Tile {
                layout: *layout,
                rect: Rect::new(
                    Point::new(
                        rect.loc.x + PADDING + index as i32 * (tile_width + TILE_GAP),
                        rect.loc.y + PADDING,
                    ),
                    Size::new(tile_width, TILE_HEIGHT),
                ),
            })
            .collect();
        Some(Bar { rect, tiles })
    }

    pub fn tiles(&self) -> &[Tile] {
        &self.tiles
    }

    pub fn shown(&self, spread: f32) -> Rect<f64, Logical> {
        let grown = f64::from(PEEK)
            + f64::from(self.rect.size.h - PEEK) * f64::from(spread.clamp(0.0, 1.0));
        let mut rect = self.rect.to_f64();
        rect.size.h = grown;
        rect
    }

    pub fn hit(&self, at: Point<f64, Logical>) -> Option<Hit> {
        self.tiles.iter().enumerate().find_map(|(index, tile)| {
            let zone = tile
                .layout
                .zones(tile.rect)
                .position(|zone| zone.to_f64().contains(at))?;
            Some(Hit { tile: index, zone })
        })
    }

    pub fn revealed(&self, at: Point<f64, Logical>, was: Reveal) -> Reveal {
        match was {
            Reveal::Hidden if self.wakes(at) => Reveal::Peek,
            Reveal::Hidden => Reveal::Hidden,
            Reveal::Peek if self.opens(at) => Reveal::Open,
            Reveal::Open if self.holds(at) => Reveal::Open,
            _ if self.wakes(at) => Reveal::Peek,
            _ => Reveal::Hidden,
        }
    }

    fn opens(&self, at: Point<f64, Logical>) -> bool {
        self.rect.to_f64().contains(at)
    }

    fn holds(&self, at: Point<f64, Logical>) -> bool {
        let rect = self.rect.to_f64();
        at.x >= rect.loc.x - GRACE
            && at.x <= rect.loc.x + rect.size.w + GRACE
            && at.y <= rect.loc.y + rect.size.h + GRACE
    }

    fn wakes(&self, at: Point<f64, Logical>) -> bool {
        let sliver = self.shown(0.0);
        at.y <= sliver.loc.y + sliver.size.h + REVEAL_BAND
            && at.x >= sliver.loc.x - REVEAL_SPREAD
            && at.x <= sliver.loc.x + sliver.size.w + REVEAL_SPREAD
    }
}

fn inset_tile(tile: Rect<i32, Logical>) -> Rect<i32, Logical> {
    Rect::new(
        tile.loc + Point::new(TILE_INSET, TILE_INSET),
        Size::new(
            (tile.size.w - 2 * TILE_INSET).max(1),
            (tile.size.h - 2 * TILE_INSET).max(1),
        ),
    )
}

fn inset(zone: Rect<i32, Logical>) -> Rect<i32, Logical> {
    Rect::new(
        zone.loc + Point::new(ZONE_GAP / 2, ZONE_GAP / 2),
        Size::new(
            (zone.size.w - ZONE_GAP).max(1),
            (zone.size.h - ZONE_GAP).max(1),
        ),
    )
}
