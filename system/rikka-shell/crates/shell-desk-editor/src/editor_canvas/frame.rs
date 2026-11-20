use rikka_types::{Point, Rect, RoundedRect, Size};
use shell_kit::taskbar::Shape;
use shell_wallpaper::stage::{aspect, below_taskbar, lerp_rect};

pub(crate) const PANEL_WIDTH: f32 = 400.0;
pub(crate) const EDGE: f32 = 16.0;

const THUMBS_TOP: f32 = 16.0;
const THUMB_SHARE: f32 = 0.085;
const THUMB_LEAST: f32 = 48.0;
const THUMB_MOST: f32 = 84.0;
const THUMB_GAP: f32 = 12.0;
const THUMBS_SIDE: f32 = 24.0;
const CARD_BELOW_THUMBS: f32 = 28.0;
const CARD_BOTTOM: f32 = 32.0;
const CARD_SIDE: f32 = 40.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    pub(crate) stage: RoundedRect,
    output: Rect,
    room: Rect,
    card: Rect,
    thumb: Size,
    across: f32,
}

impl Frame {
    #[must_use]
    pub(crate) fn new(bounds: Rect, taskbar: Option<Shape>, count: usize) -> Frame {
        let stage = below_taskbar(bounds, taskbar);
        let room = Rect::new(
            stage.rect.min_x(),
            stage.rect.min_y(),
            (stage.rect.width() - PANEL_WIDTH - EDGE).max(1.0),
            stage.rect.height(),
        );
        let aspect = aspect(bounds.size);
        let tall = (room.height() * THUMB_SHARE).clamp(THUMB_LEAST, THUMB_MOST);
        let thumb = Size::new(tall * aspect, tall);
        let across = count as f32 * thumb.width + count.saturating_sub(1) as f32 * THUMB_GAP;

        let top = room.min_y() + THUMBS_TOP + tall + CARD_BELOW_THUMBS;
        let bottom = room.max_y() - CARD_BOTTOM;
        let scale = ((bottom - top).max(1.0) / bounds.height().max(1.0))
            .min((room.width() - CARD_SIDE * 2.0).max(1.0) / bounds.width().max(1.0))
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
            room,
            card,
            thumb,
            across,
        }
    }

    #[must_use]
    pub(crate) fn card(&self, spread: f32) -> Rect {
        lerp_rect(self.output, self.card, spread)
    }

    #[must_use]
    pub(crate) fn strip(&self) -> Rect {
        Rect::new(
            self.room.min_x() + THUMBS_SIDE,
            self.stage.rect.min_y() + THUMBS_TOP,
            (self.room.width() - THUMBS_SIDE * 2.0).max(1.0),
            self.thumb.height,
        )
    }

    #[must_use]
    pub(super) fn overflow(&self) -> f32 {
        (self.across - self.strip().width()).max(0.0)
    }

    #[must_use]
    pub(crate) fn thumb(&self, index: usize, spread: f32, scroll: f32) -> Rect {
        let strip = self.strip();
        let left = match self.overflow() > 0.0 {
            true => strip.min_x() - scroll,
            false => strip.center().x - self.across / 2.0,
        };
        let rise = (THUMBS_TOP + self.thumb.height) * (1.0 - spread);
        Rect::new(
            left + index as f32 * (self.thumb.width + THUMB_GAP),
            strip.min_y() - rise,
            self.thumb.width,
            self.thumb.height,
        )
    }

    #[must_use]
    pub(crate) fn showing(&self, index: usize, scroll: f32) -> f32 {
        let strip = self.strip();
        let start = index as f32 * (self.thumb.width + THUMB_GAP);
        let end = start + self.thumb.width;
        let scroll = match (start < scroll, end > scroll + strip.width()) {
            (true, _) => start - THUMB_GAP,
            (_, true) => end - strip.width() + THUMB_GAP,
            _ => scroll,
        };
        scroll.clamp(0.0, self.overflow())
    }

    #[must_use]
    pub(crate) fn output(&self) -> Rect {
        self.output
    }
}

#[must_use]
pub(crate) fn panel(output: Size, taskbar: Option<Shape>) -> Rect {
    let stage = below_taskbar(Rect::from_origin_size(Point::ZERO, output), taskbar).rect;
    Rect::new(
        stage.max_x() - PANEL_WIDTH - EDGE,
        stage.min_y() + EDGE,
        PANEL_WIDTH + EDGE,
        (stage.height() - EDGE * 2.0).max(0.0),
    )
}
