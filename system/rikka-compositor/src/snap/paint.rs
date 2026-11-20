use rikka_theme::Corner;
use rikka_types::{CornerRadii, Rect as SceneRect, RoundedRect};

use super::assist::Assist;
use super::element::Colours;
use super::state::Drag;
use crate::render::{Panel, Piece, View};

const PANEL_RADIUS: f32 = Corner::Window.radius();
const TILE_RADIUS: f32 = Corner::Item.radius();
const ZONE_RADIUS: f32 = Corner::Detail.radius();
const CARD_RADIUS: f32 = Corner::Panel.radius();
const PREVIEW_RADIUS: f32 = Corner::Panel.radius();
const TITLE: f32 = 13.0;

pub(crate) fn bar(drag: &Drag, screen: View, colours: Colours) -> Option<Panel> {
    let bar = drag.bar()?;
    let alpha = drag.shown();
    if alpha <= 0.0 {
        return None;
    }
    let room = screen.exact(bar.shown(drag.spread()));
    let mut pieces = vec![Piece::outlined(
        RoundedRect::new(
            room,
            screen.radii(CornerRadii::new(0.0, 0.0, PANEL_RADIUS, PANEL_RADIUS)),
        ),
        colours.panel,
        screen.measure(1.0),
        colours.outline,
    )];
    for (index, tile) in bar.tiles().iter().enumerate() {
        let hovered = drag.hit().filter(|hit| hit.tile == index);
        if let Some(card) = cut(
            room,
            screen.rect(tile.rect),
            screen.radii(CornerRadii::uniform(TILE_RADIUS)),
        ) {
            pieces.push(Piece::filled(card, colours.card));
            if hovered.is_some() {
                pieces.push(Piece::filled(card, colours.hover));
            }
        }
        for (number, zone) in tile.zones().enumerate() {
            let chosen = hovered.is_some_and(|hit| hit.zone == number);
            let Some(shape) = cut(
                room,
                screen.rect(zone),
                screen.radii(CornerRadii::uniform(ZONE_RADIUS)),
            ) else {
                continue;
            };
            pieces.push(Piece::filled(
                shape,
                if chosen { colours.chosen } else { colours.zone },
            ));
        }
    }
    Panel::new(pieces, alpha)
}

pub(crate) fn preview(drag: &Drag, screen: View, colours: Colours) -> Option<Panel> {
    let (rect, alpha) = drag.preview()?;
    Panel::new(
        vec![Piece::outlined(
            RoundedRect::new(
                screen.exact(rect),
                screen.radii(CornerRadii::uniform(PREVIEW_RADIUS)),
            ),
            colours.veil(false),
            screen.measure(1.0),
            colours.outline,
        )],
        alpha,
    )
}

pub(crate) fn offer(assist: &Assist, screen: View, colours: Colours) -> Option<Panel> {
    let mut pieces = Vec::new();
    for (index, card) in assist.cards().iter().enumerate() {
        let hovered = assist.hovered() == Some(index);
        pieces.push(Piece::outlined(
            RoundedRect::new(
                screen.rect(card.slot.card),
                screen.radii(CornerRadii::uniform(CARD_RADIUS)),
            ),
            if hovered {
                colours.chosen
            } else {
                colours.card
            },
            screen.measure(1.0),
            colours.outline,
        ));
        pieces.push(Piece::label(
            screen.rect(card.slot.label),
            card.title.clone(),
            screen.measure(TITLE),
            colours.ink(hovered),
        ));
    }
    Panel::new(pieces, assist.alpha())
}

fn cut(room: SceneRect, rect: SceneRect, radii: CornerRadii) -> Option<RoundedRect> {
    RoundedRect::new(rect, radii).intersect(room)
}
