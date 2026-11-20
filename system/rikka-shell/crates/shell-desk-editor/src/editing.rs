use desk_layout::{
    Area, Content, GAP, Kind, Layout, Size, Spot, Widget, WidgetId, first_vacant, vacant,
};
use rikka_seam::SeamClient;
use rikka_settings::session::appearance::UNSET;
use rikka_types::{Point, Rect};
use shell_desk::Board;
use shell_kit::appearance;

use crate::history::History;

#[derive(Clone, Debug)]
pub(crate) struct Editing {
    board: Board,
    history: History,
    seam: SeamClient,
}

impl Editing {
    #[must_use]
    pub(crate) fn new(board: Board, seam: SeamClient) -> Editing {
        Editing {
            board,
            history: History::default(),
            seam,
        }
    }

    #[must_use]
    pub(crate) fn history(&self) -> &History {
        &self.history
    }

    #[must_use]
    pub(crate) fn layout(&self) -> Layout {
        self.board.shown_layout()
    }

    #[must_use]
    pub(crate) fn fresh_id(&self) -> WidgetId {
        self.board.fresh_id()
    }

    pub(crate) fn place(&self, widget: WidgetId, rect: Rect, area: &Area) {
        self.commit(|layout| {
            if let Some(placed) = layout.get_mut(widget) {
                placed.spot = area.spot_of(rect);
            }
        });
    }

    pub(crate) fn nudge(&self, widget: WidgetId, by: Point, area: &Area) -> Option<Rect> {
        let wallpaper = self.board.showing();
        let before = self.board.layout(wallpaper.as_deref());
        let rect = area.contain(before.rect_of(widget, area)?.translate(by));
        let changed = self.board.change(wallpaper.as_deref(), |layout| {
            if let Some(moved) = layout.get_mut(widget) {
                moved.spot = area.spot_of(rect);
            }
        });
        if changed {
            self.history.record_nudge(widget, (wallpaper, before));
        }
        Some(rect)
    }

    pub(crate) fn insert(
        &self,
        preferred_id: WidgetId,
        content: Content,
        size: Size,
        rect: Rect,
        area: &Area,
    ) -> WidgetId {
        let id = preferred_id.max(self.board.fresh_id());
        let widget = Widget {
            id,
            size,
            spot: area.spot_of(rect),
            content,
        };
        self.commit(|layout| layout.widgets.push(widget));
        id
    }

    pub(crate) fn add(&self, kind: Kind, size: Size, area: &Area) -> WidgetId {
        let others = self.layout().rects(area);
        let others: Vec<Rect> = others.into_iter().map(|(_, rect)| rect).collect();
        let extent = size.extent();
        let rect = first_vacant(extent, &others, area)
            .unwrap_or_else(|| vacant(area.place_extent(extent, Spot::CENTER), &others, area));
        self.insert(self.board.fresh_id(), Content::new(kind), size, rect, area)
    }

    pub(super) fn duplicate(&self, widget: WidgetId, area: &Area) -> Option<WidgetId> {
        let layout = self.layout();
        let original = layout.get(widget)?.clone();
        let rect = layout.rect_of(widget, area)?;
        let others: Vec<Rect> = layout
            .rects(area)
            .into_iter()
            .map(|(_, rect)| rect)
            .collect();
        let candidate = rect.translate(Point::new(rect.width() + GAP, 0.0));
        let rect = vacant(candidate, &others, area);
        Some(self.insert(
            self.board.fresh_id(),
            original.content,
            original.size,
            rect,
            area,
        ))
    }

    pub(crate) fn resize(&self, widget: WidgetId, size: Size, area: &Area) {
        let layout = self.layout();
        let Some(rect) = layout.rect_of(widget, area) else {
            return;
        };
        let fits = layout
            .get(widget)
            .is_some_and(|shown| shown.kind().fits(size));
        if !fits {
            return;
        }
        let others = layout.rects_except(widget, area);
        let extent = size.extent();
        let grown = Rect::new(rect.min_x(), rect.min_y(), extent.width, extent.height);
        let rect = vacant(grown, &others, area);
        self.commit(|layout| {
            if let Some(resized) = layout.get_mut(widget) {
                resized.size = size;
                resized.spot = area.spot_of(rect);
            }
        });
    }

    pub(super) fn restyle(&self, widget: WidgetId, content: Content) {
        self.commit(|layout| {
            if let Some(styled) = layout.get_mut(widget)
                && styled.kind() == content.kind()
            {
                styled.content = content;
            }
        });
    }

    pub(crate) fn remove(&self, widget: WidgetId) {
        self.commit(|layout| {
            layout.remove(widget);
        });
    }

    pub(crate) fn clear(&self) {
        self.commit(|layout| layout.widgets.clear());
    }

    pub(crate) fn undo(&self) {
        let step = self.history.undo(|wallpaper| self.board.layout(wallpaper));
        self.restore(step);
    }

    pub(crate) fn redo(&self) {
        let step = self.history.redo(|wallpaper| self.board.layout(wallpaper));
        self.restore(step);
    }

    fn restore(&self, step: Option<(Option<String>, Layout)>) {
        let Some((wallpaper, layout)) = step else {
            return;
        };
        self.board
            .change(wallpaper.as_deref(), |current| *current = layout);
        if self.board.showing() != wallpaper {
            let id = wallpaper.unwrap_or_else(|| UNSET.to_owned());
            self.seam
                .notify(appearance::write(appearance::WALLPAPER, id));
        }
    }

    fn commit(&self, edit: impl FnOnce(&mut Layout)) {
        let wallpaper = self.board.showing();
        let before = self.board.layout(wallpaper.as_deref());
        if self.board.change(wallpaper.as_deref(), edit) {
            self.history.record((wallpaper, before));
        }
    }
}
