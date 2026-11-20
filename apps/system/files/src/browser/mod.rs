mod layout;
mod lifted;
mod row;
mod tile;

use std::cell::RefCell;
use std::collections::HashSet;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use files_core::{SortKey, Sorting, ViewMode};
use files_thumbs::Thumbnail;
use rikka_app::prelude::{Event as Input, *};
use rikka_ui::{
    Accept, DragAction, DragActions, DragOffer, DragStart, Element, Handler, IntoHandler, Offer,
    Place, URI_LIST, View,
};
use rikka_widgets::{Surface, column};

use lifted::{Art, lifted};

pub(crate) use row::Row;

use layout::{Column, ROW, TILE, TILE_LINES, tile_height};
use tile::Cell as Slot;

const GUTTER: f32 = 26.0;

#[derive(Clone, Debug)]
pub(crate) struct Rename {
    pub(crate) index: usize,
    pub(crate) document: TextDocument,
    pub(crate) session: u64,
}

#[derive(Debug, Default)]
pub(crate) struct Band {
    drawn: Option<(Point, Point)>,
    base: HashSet<usize>,
    aim: Option<usize>,
}

pub(crate) struct Browser {
    rows: Rc<Vec<Row>>,
    thumbnails: Rc<RefCell<Vec<Option<Thumbnail>>>>,
    pub(crate) drawing: Watchers,
    scroll: Place,
    widths: ColumnWidths,
    band: Rc<RefCell<Band>>,
    chosen: Rc<HashSet<usize>>,
    focus: Option<usize>,
    mode: ViewMode,
    sorting: Sorting,
    rename: Option<Rename>,
    on_open: Option<Handler<(usize,)>>,
    on_open_tab: Option<Handler<(usize,)>>,
    on_select: Option<Handler<(HashSet<usize>, Option<usize>)>>,
    on_context: Option<Handler<(Point,)>>,
    on_sort: Option<Handler<(SortKey,)>>,
    on_visible: Option<Handler<(Range<usize>,)>>,
    on_rename: Option<Handler<(String,)>>,
    on_rename_cancel: Option<Handler>,
    on_drop_into: Option<Handler<(usize, DragOffer, DragAction)>>,
}

impl Browser {
    pub(crate) fn new(
        rows: Rc<Vec<Row>>,
        thumbnails: Rc<RefCell<Vec<Option<Thumbnail>>>>,
        drawing: Watchers,
        scroll: Place,
        widths: ColumnWidths,
        band: Rc<RefCell<Band>>,
    ) -> Self {
        Self {
            rows,
            thumbnails,
            drawing,
            scroll,
            widths,
            band,
            chosen: Rc::new(HashSet::new()),
            focus: None,
            mode: ViewMode::default(),
            sorting: Sorting::new(),
            rename: None,
            on_open: None,
            on_open_tab: None,
            on_select: None,
            on_context: None,
            on_sort: None,
            on_visible: None,
            on_rename: None,
            on_rename_cancel: None,
            on_drop_into: None,
        }
    }

    #[must_use]
    pub(crate) fn showing(mut self, mode: ViewMode, sorting: Sorting) -> Self {
        self.mode = mode;
        self.sorting = sorting;
        self
    }

    #[must_use]
    pub(crate) fn chosen(mut self, chosen: Rc<HashSet<usize>>, focus: Option<usize>) -> Self {
        self.chosen = chosen;
        self.focus = focus;
        self
    }

    #[must_use]
    pub(crate) fn renaming(mut self, rename: Option<Rename>) -> Self {
        self.rename = rename;
        self
    }

    #[must_use]
    pub(crate) fn on_open<M>(mut self, on_open: impl IntoHandler<(usize,), (), M>) -> Self {
        self.on_open = Some(on_open.into_handler());
        self
    }

    #[must_use]
    pub(crate) fn on_open_tab<M>(mut self, on_open_tab: impl IntoHandler<(usize,), (), M>) -> Self {
        self.on_open_tab = Some(on_open_tab.into_handler());
        self
    }

    #[must_use]
    pub(crate) fn on_select<M>(
        mut self,
        on_select: impl IntoHandler<(HashSet<usize>, Option<usize>), (), M>,
    ) -> Self {
        self.on_select = Some(on_select.into_handler());
        self
    }

    #[must_use]
    pub(crate) fn on_context<M>(mut self, on_context: impl IntoHandler<(Point,), (), M>) -> Self {
        self.on_context = Some(on_context.into_handler());
        self
    }

    #[must_use]
    pub(crate) fn on_sort<M>(mut self, on_sort: impl IntoHandler<(SortKey,), (), M>) -> Self {
        self.on_sort = Some(on_sort.into_handler());
        self
    }

    #[must_use]
    pub(crate) fn on_visible<M>(
        mut self,
        on_visible: impl IntoHandler<(Range<usize>,), (), M>,
    ) -> Self {
        self.on_visible = Some(on_visible.into_handler());
        self
    }

    #[must_use]
    pub(crate) fn on_rename<M>(mut self, on_rename: impl IntoHandler<(String,), (), M>) -> Self {
        self.on_rename = Some(on_rename.into_handler());
        self
    }

    #[must_use]
    pub(crate) fn on_rename_cancel<M>(
        mut self,
        on_rename_cancel: impl IntoHandler<(), (), M>,
    ) -> Self {
        self.on_rename_cancel = Some(on_rename_cancel.into_handler());
        self
    }

    #[must_use]
    pub(crate) fn on_drop_into<M>(
        mut self,
        on_drop_into: impl IntoHandler<(usize, DragOffer, DragAction), (), M>,
    ) -> Self {
        self.on_drop_into = Some(on_drop_into.into_handler());
        self
    }

    fn open(&self, index: usize, ctx: &mut EventCtx<'_>) {
        if let Some(on_open) = &self.on_open {
            ctx.call(on_open, (index,));
        }
    }

    fn open_tab(&self, index: usize, ctx: &mut EventCtx<'_>) {
        if let Some(on_open_tab) = &self.on_open_tab {
            ctx.call(on_open_tab, (index,));
        }
    }

    fn select(&self, chosen: HashSet<usize>, focus: Option<usize>, ctx: &mut EventCtx<'_>) {
        if let Some(on_select) = &self.on_select {
            ctx.call(on_select, (chosen, focus));
        }
    }

    fn rename(&self, name: String, ctx: &mut EventCtx<'_>) {
        if let Some(on_rename) = &self.on_rename {
            ctx.call(on_rename, (name,));
        }
    }

    fn finish_rename(&self, ctx: &mut EventCtx<'_>) {
        if let Some(rename) = &self.rename {
            self.rename(rename.document.text(), ctx);
        }
    }

    fn rename_cancel(&self, ctx: &mut EventCtx<'_>) {
        if let Some(on_rename_cancel) = &self.on_rename_cancel {
            ctx.call(on_rename_cancel, ());
        }
    }
}

impl View for Browser {
    fn build(self) -> Element {
        let (mode, count, focus) = (self.mode, self.rows.len(), self.focus);
        let renaming = self.rename.is_some();
        let widths = self.widths.clone();
        let sorting = self.sorting;
        let on_sort = self.on_sort.clone();
        let on_visible = self.on_visible.clone();
        let on_focus = self.on_select.clone().map(|on_select| {
            on_select.map(move |(index, modifiers): (usize, Modifiers)| {
                let chosen = match modifiers.shift {
                    true => run_to(focus, index),
                    false => HashSet::from([index]),
                };
                (chosen, Some(index))
            })
        });
        let browser = Rc::new(self);
        let (drawn, handled) = (Rc::clone(&browser), Rc::clone(&browser));

        let height = match mode {
            ViewMode::Details => ROW,
            ViewMode::Icons => tile_height(TILE_LINES),
        };
        let mut list = lazy_column(count, move |slot| {
            Slot {
                browser: Rc::clone(&drawn),
                index: slot.index,
            }
            .element(height)
        })
        .uniform(height)
        .focus(focus)
        .autofocus(true)
        .takes_tab(renaming)
        .place(&browser.scroll)
        .on_pointer(move |event, ctx, surface| {
            handled.typed(event, ctx)
                || handled.contextual(event, ctx, surface)
                || handled.dropping(event, ctx, surface)
                || handled.dragged(event, ctx, surface)
        })
        .grow(1.0)
        .fill_width();
        if let Some(on_visible) = on_visible {
            list = list.on_visible(on_visible);
        }
        if let Some(on_focus) = on_focus {
            list = list.on_focus(on_focus);
        }

        if mode == ViewMode::Icons {
            return list
                .tiles(Length::px(TILE))
                .gap(Space::ROOMY)
                .padding(Space::NORMAL)
                .build();
        }

        let sorted = Column::ALL
            .iter()
            .position(|column| column.sort_key() == sorting.key);
        let mut head = column_header(Column::ALL.map(Column::spec), &widths)
            .sorted(sorted, sorting.order == files_core::SortOrder::Descending)
            .gutter(GUTTER);
        if let Some(on_sort) = on_sort {
            head = head.on_sort(on_sort.map(|(at,): (usize,)| {
                (Column::ALL
                    .get(at)
                    .copied()
                    .unwrap_or(Column::Name)
                    .sort_key(),)
            }));
        }

        column! {
            head.build().fill_width().height(COLUMN_HEADER);
            list;
        }
        .fill()
        .build()
    }
}

impl Browser {
    fn typed(&self, event: &Input, ctx: &mut EventCtx<'_>) -> bool {
        let Input::KeyPressed { key, modifiers, .. } = *event else {
            return false;
        };
        let handled = self.key(key, modifiers, ctx);
        if handled {
            ctx.set_handled();
        }
        handled
    }

    fn contextual(&self, event: &Input, ctx: &mut EventCtx<'_>, surface: &Surface<'_>) -> bool {
        let Input::PointerPressed {
            position,
            button: PointerButton::Secondary,
            ..
        } = *event
        else {
            return false;
        };
        if ctx.captured() {
            return true;
        }
        ctx.request_focus();
        match hit(surface, position) {
            Some(index) if !self.chosen.contains(&index) => {
                self.select(HashSet::from([index]), Some(index), ctx);
            }
            None if !self.chosen.is_empty() || self.focus.is_some() => {
                self.select(HashSet::new(), None, ctx);
            }
            _ => {}
        }
        if let Some(on_context) = &self.on_context {
            let at = self.anchored(position, ctx.bounds());
            ctx.call(on_context, (at,));
        }
        ctx.set_handled();
        true
    }

    fn dragged(&self, event: &Input, ctx: &mut EventCtx<'_>, surface: &Surface<'_>) -> bool {
        let mut band = self.band.borrow_mut();
        match *event {
            Input::PointerPressed {
                position,
                button: PointerButton::Primary,
                modifiers,
            } => {
                ctx.request_focus();
                let base = match modifiers.control || modifiers.shift {
                    true => (*self.chosen).clone(),
                    false => HashSet::new(),
                };
                band.drawn = Some((position, position));
                band.base = base.clone();
                ctx.capture_pointer();
                self.select(base, self.focus, ctx);
                ctx.set_handled();
                true
            }
            Input::PointerMoved(position) => {
                let Some((from, _)) = band.drawn else {
                    return false;
                };
                band.drawn = Some((from, position));
                let mut chosen = band.base.clone();
                chosen.extend(banded(surface, from, position));
                self.select(chosen, self.focus, ctx);
                ctx.request_paint();
                true
            }
            Input::PointerReleased { .. } => {
                if band.drawn.take().is_none() {
                    return false;
                }
                ctx.release_pointer();
                ctx.request_paint();
                true
            }
            _ => false,
        }
    }

    fn dropping(&self, event: &Input, ctx: &mut EventCtx<'_>, surface: &Surface<'_>) -> bool {
        let Some(on_drop_into) = &self.on_drop_into else {
            return false;
        };
        match event {
            Input::DragMoved { position, offer } => {
                let aim = self.folder_at(surface, *position, offer);
                let changed = {
                    let mut band = self.band.borrow_mut();
                    std::mem::replace(&mut band.aim, aim) != aim
                };
                if changed {
                    ctx.request_paint();
                }
                let Some(accept) = aim.and_then(|_| accept_files(offer)) else {
                    return false;
                };
                ctx.accept_drag(accept);
                ctx.set_handled();
                true
            }
            Input::DragLeft => {
                if self.band.borrow_mut().aim.take().is_some() {
                    ctx.request_paint();
                }
                false
            }
            Input::DragDropped { offer, action, .. } => {
                let Some(into) = self.band.borrow_mut().aim.take() else {
                    return false;
                };
                ctx.request_paint();
                ctx.set_handled();
                ctx.call(on_drop_into, (into, offer.clone(), *action));
                true
            }
            _ => false,
        }
    }

    fn folder_at(
        &self,
        surface: &Surface<'_>,
        position: Point,
        offer: &DragOffer,
    ) -> Option<usize> {
        let index = hit(surface, position)?;
        let folder = self.rows.get(index).is_some_and(Row::is_directory);
        let carried = offer.is_ours() && self.chosen.contains(&index);
        (folder && !carried).then_some(index)
    }

    pub(crate) fn is_drop_target(&self, index: usize) -> bool {
        self.band.borrow().aim == Some(index)
    }

    pub(crate) fn start_drag(&self, index: usize, ctx: &mut EventCtx<'_>) {
        let mut carried: Vec<usize> = match self.chosen.contains(&index) {
            true => self.chosen.iter().copied().collect(),
            false => vec![index],
        };
        carried.sort_unstable();
        let rows: Vec<&Row> = carried.iter().filter_map(|at| self.rows.get(*at)).collect();
        let Some(art) = self.art(index, &ctx.theme().palette()) else {
            return;
        };
        let uris: Vec<&str> = rows.iter().map(|row| row.at.as_str()).collect();
        let written = rows
            .iter()
            .map(|row| {
                row.at.to_path().map_or_else(
                    || row.at.as_str().to_owned(),
                    |path| path.display().to_string(),
                )
            })
            .collect::<Vec<String>>()
            .join("\n");
        let offer = Offer::uris(&uris).with_text(written);
        let icon = lifted(ctx.theme().clone(), art, rows.len());
        ctx.start_drag(DragStart::new(offer, icon).actions(DragActions::COPY_OR_MOVE));
    }

    fn art(&self, index: usize, palette: &Palette) -> Option<Art> {
        let row = self.rows.get(index)?;
        if let Some(thumbnail) = self.thumbnails.borrow().get(index).and_then(Option::as_ref) {
            return Some(Art::Picture {
                path: Arc::from(thumbnail.shown()),
                pixels: thumbnail.pixels(),
            });
        }
        Some(Art::Glyph {
            symbol: row.symbol(),
            tint: faded_tint(row, palette),
        })
    }

    fn anchored(&self, position: Point, bounds: Rect) -> Point {
        let above = match self.mode {
            ViewMode::Details => COLUMN_HEADER,
            ViewMode::Icons => 0.0,
        };
        Point::new(
            position.x - bounds.origin.x,
            position.y - bounds.origin.y + above,
        )
    }

    fn after_click(&self, index: usize, modifiers: Modifiers) -> HashSet<usize> {
        if modifiers.control {
            let mut chosen = (*self.chosen).clone();
            if !chosen.remove(&index) {
                chosen.insert(index);
            }
            return chosen;
        }
        if modifiers.shift {
            return run_to(self.focus, index);
        }
        HashSet::from([index])
    }

    fn key(&self, key: Key, modifiers: Modifiers, ctx: &mut EventCtx<'_>) -> bool {
        if self.rows.is_empty() || modifiers != Modifiers::NONE {
            return false;
        }
        match key {
            Key::Enter => {
                let Some(index) = self.focus else {
                    return false;
                };
                self.open(index, ctx);
                true
            }
            Key::Escape if !self.chosen.is_empty() => {
                self.select(HashSet::new(), None, ctx);
                true
            }
            _ => false,
        }
    }
}

pub(crate) fn accept_files(offer: &DragOffer) -> Option<Accept> {
    offer
        .has_uris()
        .then(|| Accept::copy_or_move(URI_LIST, offer))
}

fn banded(surface: &Surface<'_>, from: Point, to: Point) -> HashSet<usize> {
    let band = band_rect(from, to);
    surface
        .visible()
        .filter(|&index| {
            let cell = surface.cell(index);
            cell.min_x() < band.max_x()
                && cell.max_x() > band.min_x()
                && cell.min_y() < band.max_y()
                && cell.max_y() > band.min_y()
        })
        .collect()
}

fn hit(surface: &Surface<'_>, position: Point) -> Option<usize> {
    surface
        .visible()
        .find(|&index| surface.cell(index).contains(position))
}

fn run_to(anchor: Option<usize>, index: usize) -> HashSet<usize> {
    let anchor = anchor.unwrap_or(index);
    (anchor.min(index)..=anchor.max(index)).collect()
}

fn band_rect(from: Point, to: Point) -> Rect {
    Rect::new(
        from.x.min(to.x),
        from.y.min(to.y),
        (to.x - from.x).abs(),
        (to.y - from.y).abs(),
    )
}

fn tint(row: &Row, palette: &Palette) -> Color {
    if row.broken {
        return palette.danger;
    }
    if row.is_directory() {
        return Color::rgb(0.98, 0.78, 0.36);
    }
    match row.family {
        files_core::Family::Image => palette.success,
        files_core::Family::Video => palette.accent,
        files_core::Family::Audio => palette.warning,
        files_core::Family::Archive => Color::rgb(0.85, 0.71, 0.42),
        _ => palette.text_muted,
    }
}
fn faded_tint(row: &Row, palette: &Palette) -> Color {
    let mut tint = tint(row, palette);
    if row.hidden {
        tint.a *= 0.55;
    }
    tint
}

impl std::fmt::Debug for Browser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Browser")
            .field("rows", &self.rows.len())
            .field("chosen", &self.chosen.len())
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}
