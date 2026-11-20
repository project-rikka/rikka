use desk_layout::Projection;
use rikka_types::{Point, Rect, RoundedRect, Size};
use shell_kit::taskbar::Shape;
use shell_wallpaper::stage::{aspect, below_taskbar, lerp, lerp_rect};

const THUMBS_TOP: f32 = 16.0;
const THUMB_SHARE: f32 = 0.085;
const THUMB_LEAST: f32 = 40.0;
const THUMB_MOST: f32 = 84.0;
const THUMB_GAP: f32 = 12.0;
const THUMBS_SIDE: f32 = 24.0;
const CARDS_BELOW_THUMBS: f32 = 28.0;
const CARDS_BOTTOM: f32 = 40.0;
const PEEK: f32 = 64.0;
const CARD_GAP: f32 = 32.0;
const WINDOWS_TOP: f32 = 44.0;
const WINDOWS_SIDE: f32 = 28.0;
const WINDOWS_BOTTOM: f32 = 28.0;

pub(super) const ROW_SPACING: f32 = 44.0;
pub(super) const COLUMN_SPACING: f32 = 28.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    pub(super) stage: RoundedRect,
    output: Rect,
    reserved: f32,
    card: Rect,
    thumb: Size,
    thumbs_x: f32,
}

impl Frame {
    pub(super) fn new(bounds: Rect, taskbar: Option<Shape>, count: usize) -> Frame {
        let stage = below_taskbar(bounds, taskbar);
        let reserved = stage.rect.min_y() - bounds.min_y();
        let aspect = aspect(bounds.size);
        let room = stage.rect;

        let mut tall = (room.height() * THUMB_SHARE).clamp(THUMB_LEAST, THUMB_MOST);
        let across =
            |tall: f32| count as f32 * tall * aspect + count.saturating_sub(1) as f32 * THUMB_GAP;
        let widest = (room.width() - THUMBS_SIDE * 2.0).max(1.0);
        if across(tall) > widest {
            tall *= widest / across(tall);
        }
        let thumb = Size::new(tall * aspect, tall);
        let thumbs_x = room.center().x - across(tall) / 2.0;

        let top = room.min_y() + THUMBS_TOP + tall + CARDS_BELOW_THUMBS;
        let bottom = room.max_y() - CARDS_BOTTOM;
        let scale = ((bottom - top).max(1.0) / bounds.height().max(1.0))
            .min((room.width() - (PEEK + CARD_GAP) * 2.0).max(1.0) / bounds.width().max(1.0))
            .min(1.0);
        let size = Size::new(bounds.width() * scale, bounds.height() * scale);
        let card = Rect::new(
            room.center().x - size.width / 2.0,
            top + ((bottom - top) - size.height).max(0.0) / 2.0,
            size.width,
            size.height,
        );
        Frame {
            stage,
            output: bounds,
            reserved,
            card,
            thumb,
            thumbs_x,
        }
    }

    pub(super) fn card(&self, from_focus: f32, spread: f32) -> Rect {
        let spread_rect = lerp_rect(self.output, self.card, spread);
        let pitch = lerp(self.output.width(), self.card.width() + CARD_GAP, spread);
        spread_rect.translate(Point::new(from_focus * pitch, 0.0))
    }

    pub(super) fn pitch(&self) -> f32 {
        self.card.width() + CARD_GAP
    }

    pub(super) fn thumb(&self, index: usize, spread: f32) -> Rect {
        let rise = (THUMBS_TOP + self.thumb.height) * (1.0 - spread);
        Rect::new(
            self.thumbs_x + index as f32 * (self.thumb.width + THUMB_GAP),
            self.stage.rect.min_y() + THUMBS_TOP - rise,
            self.thumb.width,
            self.thumb.height,
        )
    }

    pub(super) fn windows_area(&self) -> Rect {
        let card = self.card;
        let scale = card.height() / self.output.height().max(1.0);
        let top = self.reserved * scale + WINDOWS_TOP;
        Rect::new(
            card.min_x() + WINDOWS_SIDE,
            card.min_y() + top,
            (card.width() - WINDOWS_SIDE * 2.0).max(1.0),
            (card.height() - top - WINDOWS_BOTTOM).max(1.0),
        )
    }

    pub(super) fn settled_card(&self) -> Rect {
        self.card
    }

    pub(super) fn placed_within(&self, real: Rect, card: Rect) -> Rect {
        Projection::new(self.output, card).rect(real)
    }
}

pub(super) fn unit_of(rect: Rect, card: Rect) -> Rect {
    let (wide, tall) = (card.width().max(1.0), card.height().max(1.0));
    Rect::new(
        (rect.min_x() - card.min_x()) / wide,
        (rect.min_y() - card.min_y()) / tall,
        rect.width() / wide,
        rect.height() / tall,
    )
}

pub(super) fn from_unit(unit: Rect, card: Rect) -> Rect {
    Rect::new(
        card.min_x() + unit.min_x() * card.width(),
        card.min_y() + unit.min_y() * card.height(),
        unit.width() * card.width(),
        unit.height() * card.height(),
    )
}
