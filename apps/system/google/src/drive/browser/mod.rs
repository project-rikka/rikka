mod cell;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::rc::Rc;

use google_drive::Thumbnail;
use rikka_app::prelude::{Event as Input, *};
use rikka_ui::{
    Accept, DragAction, DragActions, DragIcon, DragOffer, DragStart, Element, Handler, IntoHandler,
    Offer, Place, URI_LIST, View,
};
use rikka_widgets::{Surface, column, row};

use crate::drive::row::{Column, Row, Sorting, ViewMode};
use cell::Cell;

pub(crate) const ITEMS: &str = "application/x-rikka-google-drive-items";

pub(crate) const ROW: f32 = 30.0;
pub(crate) const TILE: f32 = 124.0;
pub(crate) const TILE_ART: f32 = 88.0;
pub(crate) const TILE_LINE: f32 = 17.0;
pub(crate) const TILE_LINES: usize = 2;
pub(crate) const GUTTER: f32 = 28.0;

const LIFTED: Size = Size::new(220.0, 40.0);

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

pub(crate) type Thumbnails = Rc<RefCell<HashMap<String, Thumbnail>>>;

pub(crate) struct Browser {
    rows: Rc<Vec<Row>>,
    thumbnails: Thumbnails,
    drawing: Watchers,
    scroll: Place,
    widths: ColumnWidths,
    band: Rc<RefCell<Band>>,
    chosen: Rc<HashSet<usize>>,
    focus: Option<usize>,
    mode: ViewMode,
    sorting: Sorting,
    rename: Option<Rename>,
    on_open: Option<Handler<(usize,)>>,
    on_select: Option<Handler<(HashSet<usize>, Option<usize>)>>,
    on_context: Option<Handler<(Point,)>>,
    on_sort: Option<Handler<(Column,)>>,
    on_visible: Option<Handler<(Range<usize>,)>>,
    on_rename: Option<Handler<(String,)>>,
    on_rename_cancel: Option<Handler>,
    on_drop_into: Option<Handler<(usize, DragOffer, DragAction)>>,
}

impl Browser {
    pub(crate) fn new(
        rows: Rc<Vec<Row>>,
        thumbnails: Thumbnails,
        drawing: Watchers,
        scroll: Place,
        widths: ColumnWidths,
        band: Rc<RefCell<Band>>,
    ) -> Browser {
        Browser {
            rows,
            thumbnails,
            drawing,
            scroll,
            widths,
            band,
            chosen: Rc::new(HashSet::new()),
            focus: None,
            mode: ViewMode::Details,
            sorting: Sorting::default(),
            rename: None,
            on_open: None,
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
    pub(crate) fn showing(mut self, mode: ViewMode, sorting: Sorting) -> Browser {
        self.mode = mode;
        self.sorting = sorting;
        self
    }

    #[must_use]
    pub(crate) fn chosen(mut self, chosen: Rc<HashSet<usize>>, focus: Option<usize>) -> Browser {
        self.chosen = chosen;
        self.focus = focus;
        self
    }

    #[must_use]
    pub(crate) fn renaming(mut self, rename: Option<Rename>) -> Browser {
        self.rename = rename;
        self
    }

    #[must_use]
    pub(crate) fn on_open<M>(mut self, on_open: impl IntoHandler<(usize,), (), M>) -> Browser {
        self.on_open = Some(on_open.into_handler());
        self
    }

    #[must_use]
    pub(crate) fn on_select<M>(
        mut self,
        on_select: impl IntoHandler<(HashSet<usize>, Option<usize>), (), M>,
    ) -> Browser {
        self.on_select = Some(on_select.into_handler());
        self
    }

    #[must_use]
    pub(crate) fn on_context<M>(
        mut self,
        on_context: impl IntoHandler<(Point,), (), M>,
    ) -> Browser {
        self.on_context = Some(on_context.into_handler());
        self
    }

    #[must_use]
    pub(crate) fn on_sort<M>(mut self, on_sort: impl IntoHandler<(Column,), (), M>) -> Browser {
        self.on_sort = Some(on_sort.into_handler());
        self
    }

    #[must_use]
    pub(crate) fn on_visible<M>(
        mut self,
        on_visible: impl IntoHandler<(Range<usize>,), (), M>,
    ) -> Browser {
        self.on_visible = Some(on_visible.into_handler());
        self
    }

    #[must_use]
    pub(crate) fn on_rename<M>(mut self, on_rename: impl IntoHandler<(String,), (), M>) -> Browser {
        self.on_rename = Some(on_rename.into_handler());
        self
    }

    #[must_use]
    pub(crate) fn on_rename_cancel<M>(
        mut self,
        on_rename_cancel: impl IntoHandler<(), (), M>,
    ) -> Browser {
        self.on_rename_cancel = Some(on_rename_cancel.into_handler());
        self
    }

    #[must_use]
    pub(crate) fn on_drop_into<M>(
        mut self,
        on_drop_into: impl IntoHandler<(usize, DragOffer, DragAction), (), M>,
    ) -> Browser {
        self.on_drop_into = Some(on_drop_into.into_handler());
        self
    }

    fn open(&self, index: usize, ctx: &mut EventCtx<'_>) {
        if let Some(on_open) = &self.on_open {
            ctx.call(on_open, (index,));
        }
    }

    fn select(&self, chosen: HashSet<usize>, focus: Option<usize>, ctx: &mut EventCtx<'_>) {
        if let Some(on_select) = &self.on_select {
            ctx.call(on_select, (chosen, focus));
        }
    }

    fn finish_rename(&self, ctx: &mut EventCtx<'_>) {
        if let (Some(rename), Some(on_rename)) = (&self.rename, &self.on_rename) {
            ctx.call(on_rename, (rename.document.text(),));
        }
    }

    fn rename_cancel(&self, ctx: &mut EventCtx<'_>) {
        if let Some(on_rename_cancel) = &self.on_rename_cancel {
            ctx.call(on_rename_cancel, ());
        }
    }

    fn is_drop_target(&self, index: usize) -> bool {
        self.band.borrow().aim == Some(index)
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

    fn start_drag(&self, index: usize, ctx: &mut EventCtx<'_>) {
        let mut carried: Vec<usize> = match self.chosen.contains(&index) {
            true => self.chosen.iter().copied().collect(),
            false => vec![index],
        };
        carried.sort_unstable();
        let rows: Vec<&Row> = carried.iter().filter_map(|at| self.rows.get(*at)).collect();
        let Some(held) = self.rows.get(index) else {
            return;
        };
        let ids = rows
            .iter()
            .map(|row| row.file.id.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let names = rows
            .iter()
            .map(|row| row.file.name.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let offer = Offer::text(names).with(ITEMS, ids.into_bytes());
        let label = match rows.len() {
            1 => Label::from(held.file.name.clone()),
            count => tr!("google.drive.items", count = count),
        };
        let (symbol, tint) = (held.symbol(), held.tint(&ctx.theme().palette()));
        let icon = DragIcon::new(
            LIFTED,
            Point::new(16.0, LIFTED.height / 2.0),
            ctx.theme().clone(),
            move || {
                container(
                    row! {
                        icon(symbol).extent(Length::px(20.0)).color(tint);
                        text(label.clone()).label().no_wrap().grow(1.0).min_width(0.0);
                    }
                    .gap(Space::SNUG)
                    .align_items(Align::Center),
                )
                .padding(Inset::xy(Space::NORMAL, Space::SNUG))
                .surface()
                .radius(Corner::Panel)
                .fill()
                .build()
            },
        );
        ctx.start_drag(DragStart::new(offer, icon).actions(DragActions::MOVE));
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
            ViewMode::Icons => tile_height(),
        };
        let mut list = lazy_column(count, move |slot| {
            Cell {
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
        let sorted = sorting
            .column
            .and_then(|column| Column::ALL.iter().position(|each| *each == column));
        let mut head = column_header(Column::ALL.map(Column::spec), &widths)
            .sorted(sorted, sorting.descending)
            .gutter(GUTTER);
        if let Some(on_sort) = on_sort {
            head = head
                .on_sort(on_sort.map(|(at,): (usize,)| {
                    (Column::ALL.get(at).copied().unwrap_or(Column::Name),)
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
        if self.rows.is_empty() || modifiers != Modifiers::NONE {
            return false;
        }
        let handled = match key {
            Key::Enter => match self.focus {
                Some(index) => {
                    self.open(index, ctx);
                    true
                }
                None => false,
            },
            Key::Escape if !self.chosen.is_empty() => {
                self.select(HashSet::new(), None, ctx);
                true
            }
            _ => false,
        };
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
            let above = match self.mode {
                ViewMode::Details => COLUMN_HEADER,
                ViewMode::Icons => 0.0,
            };
            let bounds = ctx.bounds();
            let at = Point::new(
                position.x - bounds.origin.x,
                position.y - bounds.origin.y + above,
            );
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
                band.base.clone_from(&base);
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
                let Some(accept) = aim.and_then(|_| accepted(offer)) else {
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
        let row = self.rows.get(index)?;
        let takes = row.file.is_folder() && row.file.capabilities.can_add_children;
        let carried = offer.is_ours() && offer.offers(ITEMS) && self.chosen.contains(&index);
        (takes && !carried).then_some(index)
    }
}

pub(crate) fn accepted(offer: &DragOffer) -> Option<Accept> {
    if offer.is_ours() && offer.offers(ITEMS) {
        return Some(Accept::moving(ITEMS));
    }
    offer.has_uris().then(|| Accept::copy(URI_LIST))
}

pub(crate) const fn tile_height() -> f32 {
    TILE_ART + 4.0 + TILE_LINES as f32 * TILE_LINE + 6.0
}

fn banded(surface: &Surface<'_>, from: Point, to: Point) -> HashSet<usize> {
    let band = Rect::new(
        from.x.min(to.x),
        from.y.min(to.y),
        (to.x - from.x).abs(),
        (to.y - from.y).abs(),
    );
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

impl std::fmt::Debug for Browser {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Browser")
            .field("rows", &self.rows.len())
            .field("chosen", &self.chosen.len())
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}
