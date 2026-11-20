use std::ops::Range;

use rikka_symbols::MaterialSymbol;
use std::sync::atomic::{AtomicU64, Ordering};

use rikka_types::{Point, PointerButton, Rect, Size};
use rikka_ui::text::Label;
use rikka_ui::{
    Action, Behavior, DropdownPlacement, Event, EventCtx, IntoHandler, Layout, PaintCtx, Style,
    Widget, drop_panel,
};

use crate::controls::track::TrackState;
use crate::navigation::dropdown::{DropdownState, Trigger, TriggerProps};
use crate::navigation::group_tabs::{Group, GroupTabs};
use crate::navigation::menu::{Cascade, Keyed, Laid, MenuItem, Panel, Pointed, Press, menu_item};
use crate::rows;

const DROP: f32 = 4.0;

#[must_use]
#[derive(Widget)]
pub struct Combo {
    trigger: TriggerProps,
    groups: Vec<Group>,
    more: Option<More>,
    on_select: Action<(usize,)>,
    focusless: bool,
    made: u64,
    #[style]
    style: Style,
}

struct More {
    first: usize,
    label: Label,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PanelState {
    tab: usize,
    folded: Option<usize>,
}

pub fn combo(selected: impl Into<Option<usize>>) -> Combo {
    Combo {
        trigger: TriggerProps::new(selected.into()),
        groups: Vec::new(),
        more: None,
        on_select: Action::default(),
        focusless: false,
        made: MADE.fetch_add(1, Ordering::Relaxed),
        style: Style::default(),
    }
}

static MADE: AtomicU64 = AtomicU64::new(1);

impl Combo {
    crate::navigation::dropdown::trigger_methods!();

    pub fn group<L: Into<Label>>(
        mut self,
        label: impl Into<Label>,
        options: impl IntoIterator<Item = L>,
    ) -> Self {
        let from = self.trigger.options.len();
        self.trigger
            .options
            .extend(options.into_iter().map(Into::into));
        self.groups.push(Group {
            label: label.into(),
            span: from..self.trigger.options.len(),
        });
        self
    }

    pub fn more(mut self, first: usize, label: impl Into<Label>) -> Self {
        self.more = Some(More {
            first,
            label: label.into(),
        });
        self
    }

    pub fn on_select<Marker>(mut self, on_select: impl IntoHandler<(usize,), (), Marker>) -> Self {
        self.on_select = Action::new(on_select);
        self
    }

    pub fn focusless(mut self) -> Self {
        self.focusless = true;
        self
    }

    fn is_active(&self) -> bool {
        self.trigger.enabled && !self.trigger.options.is_empty()
    }

    fn face(&self, open: bool) -> Trigger<'_> {
        self.trigger.face(open, self.is_active())
    }

    fn grouped(&self) -> Option<&[Group]> {
        let covered = self.groups.iter().try_fold(0, |next, group| {
            (group.span.start == next).then_some(group.span.end)
        });
        (self.groups.len() > 1 && covered == Some(self.trigger.options.len()))
            .then_some(self.groups.as_slice())
    }

    fn tabs(&self, shown: usize) -> Option<GroupTabs<'_>> {
        self.grouped().map(|groups| GroupTabs {
            groups,
            shown,
            size: self.trigger.size,
        })
    }

    fn showing(&self, state: &ComboState) -> PanelState {
        let holds = |group: &Group| {
            self.trigger
                .selected
                .is_some_and(|at| group.span.contains(&at))
        };
        let tab = state
            .tab
            .filter(|tab| *tab < self.groups.len())
            .or_else(|| self.groups.iter().position(holds))
            .unwrap_or_default();
        let folded = self
            .more
            .as_ref()
            .filter(|_| !state.unfolded && self.grouped().is_none())
            .map(|more| more.first)
            .filter(|first| *first < self.trigger.options.len())
            .filter(|first| self.trigger.selected.is_none_or(|at| at < *first));
        PanelState { tab, folded }
    }

    fn span(&self, showing: PanelState) -> Range<usize> {
        match (self.grouped(), showing.folded) {
            (Some(groups), _) => groups[showing.tab].span.clone(),
            (None, Some(first)) => 0..first,
            (None, None) => 0..self.trigger.options.len(),
        }
    }

    fn listed(&self, showing: PanelState, option: Option<usize>) -> Option<usize> {
        let span = self.span(showing);
        option
            .filter(|option| span.contains(option))
            .map(|option| option - span.start)
    }

    fn picked(&self, showing: PanelState, row: Option<usize>) -> Choice {
        let span = self.span(showing);
        match row {
            Some(row) if showing.folded.is_some() && row == span.len() => Choice::More,
            Some(row) => Choice::Option(Some(span.start + row)),
            None => Choice::Option(None),
        }
    }

    fn items(&self, showing: PanelState) -> Vec<MenuItem> {
        let selected = self.trigger.selected;
        let span = self.span(showing);
        let mut items: Vec<MenuItem> = self.trigger.options[span.clone()]
            .iter()
            .enumerate()
            .map(|(row, label)| {
                menu_item(label.clone()).selected(Some(span.start + row) == selected)
            })
            .collect();
        if let (Some(more), Some(_)) = (&self.more, showing.folded) {
            items.push(menu_item(more.label.clone()).icon(MaterialSymbol::EXPAND_MORE));
        }
        items
    }

    fn panel<'s>(
        &self,
        showing: PanelState,
        listed: &'s mut Option<Listed>,
        trigger: Rect,
        viewport: Size,
    ) -> (Panel<'s>, DropdownPlacement) {
        let key = (self.made, showing);
        let fresh = listed
            .take()
            .filter(|listed| listed.key == key)
            .unwrap_or_else(|| Listed {
                key,
                items: self.items(showing),
            });
        let items = &listed.insert(fresh).items;
        let grouped = self.grouped();
        let panel = Panel {
            items,
            size: self.trigger.size,
            header: grouped.map(|_| GroupTabs::height(self.trigger.size)),
        };
        let width = match grouped {
            Some(groups) => trigger.width().max(GroupTabs::width(groups.len())),
            None => trigger.width(),
        };
        let dropped = drop_panel(
            trigger,
            DROP,
            Size::new(width, panel.total_height()),
            viewport,
        );
        (panel, dropped)
    }

    fn laid<'s>(
        &self,
        showing: PanelState,
        listed: &'s mut Option<Listed>,
        cascade: &mut Cascade,
        ctx: &EventCtx<'_>,
    ) -> (Laid<'s>, DropdownPlacement) {
        let (panel, dropped) = self.panel(showing, listed, ctx.bounds(), ctx.viewport());
        let laid = cascade.lay(panel, dropped.rect(), ctx.theme(), ctx.viewport());
        (laid, dropped)
    }

    fn highlight(&self, state: &mut ComboState, row: Option<usize>, ctx: &EventCtx<'_>) {
        let showing = self.showing(state);
        let ComboState {
            listed, cascade, ..
        } = state;
        let (laid, _) = self.laid(showing, listed, cascade, ctx);
        match row {
            Some(row) => cascade.highlight(&laid, row),
            None => {
                cascade.root_mut().highlight(None);
            }
        }
    }

    fn open(&self, state: &mut ComboState, ctx: &mut EventCtx<'_>) {
        state.open = true;
        state.cascade.relist();
        let at = self.listed(self.showing(state), self.trigger.selected);
        self.highlight(state, at, ctx);
        ctx.capture_pointer_exclusively();
        ctx.request_paint();
    }

    fn tab_to(&self, state: &mut ComboState, tab: usize, ctx: &mut EventCtx<'_>) {
        if state.tab == Some(tab) {
            return;
        }
        state.tab = Some(tab);
        state.cascade.relist();
        ctx.request_paint();
    }

    fn unfold(&self, state: &mut ComboState, row: usize, ctx: &mut EventCtx<'_>) {
        state.unfolded = true;
        state.cascade.relist();
        self.highlight(state, Some(row), ctx);
        ctx.request_paint();
    }

    fn choose(
        &self,
        state: &mut ComboState,
        showing: PanelState,
        row: Option<usize>,
        ctx: &mut EventCtx<'_>,
    ) {
        match self.picked(showing, row) {
            Choice::More => self.unfold(state, self.span(showing).len(), ctx),
            Choice::Option(picked) => {
                self.close(state, ctx);
                rows::select(ctx, &self.on_select, self.trigger.selected, picked);
            }
        }
    }

    fn close(&self, state: &mut ComboState, ctx: &mut EventCtx<'_>) {
        state.open = false;
        state.tab = None;
        state.unfolded = false;
        state.tabs.hover(None);
        ctx.release_pointer();
        ctx.request_paint();
    }

    fn dispatch(&self, state: &mut ComboState, event: &Event, ctx: &mut EventCtx<'_>) -> Act {
        let showing = self.showing(state);
        let tabs = self.tabs(showing.tab);
        let ComboState {
            listed,
            cascade,
            tabs: strip,
            ..
        } = state;
        let (laid, dropped) = self.laid(showing, listed, cascade, ctx);
        let tab_at = |at: Point| tabs.as_ref().and_then(|tabs| tabs.at(dropped.rect(), at));
        match *event {
            Event::PointerMoved(at) => {
                ctx.set_handled();
                let hovered = strip.hover(tab_at(at));
                let pointed = cascade.pointer(&laid, event) == Some(Pointed::Changed);
                Act::painted(hovered | pointed)
            }
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                ctx.set_handled();
                if let Some(tab) = tab_at(position) {
                    return Act::Tab(tab);
                }
                match cascade.pointer(&laid, event) {
                    Some(Pointed::Pressed(Press::Leaf { row, .. })) => Act::Choose(Some(row)),
                    _ => Act::Choose(None),
                }
            }
            Event::Scroll { .. } => {
                ctx.set_handled();
                Act::painted(cascade.pointer(&laid, event) == Some(Pointed::Changed))
            }
            Event::PointerReleased { .. } => {
                ctx.set_handled();
                Act::Kept
            }
            Event::KeyPressed { key, modifiers, .. } if ctx.focused() => {
                let act = match cascade.key(&laid, key, modifiers) {
                    Keyed::Ignored => return Act::Kept,
                    Keyed::Aside { forward } => match tabs {
                        Some(tabs) => {
                            let last = tabs.groups.len() - 1;
                            Act::Tab(match forward {
                                true => (showing.tab + 1).min(last),
                                false => showing.tab.saturating_sub(1),
                            })
                        }
                        None => return Act::Kept,
                    },
                    Keyed::Kept | Keyed::Slid { .. } => Act::Kept,
                    Keyed::Changed => Act::Paint,
                    Keyed::Choose { row, .. } => Act::Choose(Some(row)),
                    Keyed::Close => Act::Close,
                };
                ctx.set_handled();
                act
            }
            _ => Act::Kept,
        }
    }
}

enum Act {
    Kept,
    Paint,
    Tab(usize),
    Choose(Option<usize>),
    Close,
}

impl Act {
    fn painted(changed: bool) -> Act {
        match changed {
            true => Act::Paint,
            false => Act::Kept,
        }
    }
}

struct Listed {
    key: (u64, PanelState),
    items: Vec<MenuItem>,
}

impl std::fmt::Debug for Listed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Listed")
            .field("items", &self.items.len())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Choice {
    More,
    Option(Option<usize>),
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct ComboState {
    open: bool,
    tab: Option<usize>,
    unfolded: bool,
    trigger: DropdownState,
    cascade: Cascade,
    listed: Option<Listed>,
    tabs: TrackState,
}

impl Widget for Combo {
    type State = ComboState;

    fn layout(&self) -> Layout {
        self.face(false).layout(&self.style)
    }

    fn behavior(&self, state: &Self::State) -> Behavior {
        Behavior::OVERLAY.when(state.open || state.cascade.root().shown() > 0.0)
            | Behavior::TRACKS_HOVER
            | Behavior::FOCUSABLE
                .when(self.on_select.active(self.trigger.enabled) && !self.focusless)
    }

    fn paint(&self, state: &mut ComboState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let open = state.open;
        self.face(open).paint(ctx, &mut state.trigger, bounds);
        let reveal = state.cascade.root_mut().reveal(ctx, open);
        if reveal <= 0.0 {
            return;
        }
        let showing = self.showing(state);
        let tabs = self.tabs(showing.tab);
        let ComboState {
            listed,
            cascade,
            tabs: strip,
            ..
        } = state;
        let (panel, dropped) = self.panel(showing, listed, bounds, ctx.viewport());
        panel.paint(
            ctx,
            cascade.root_mut(),
            dropped.rect(),
            reveal,
            dropped.pivot(),
            |ctx, band| {
                if let Some(tabs) = tabs {
                    tabs.paint(ctx, strip, band);
                }
            },
        );
    }

    fn event(&self, state: &mut ComboState, event: &Event, ctx: &mut EventCtx<'_>) {
        if self.focusless && matches!(event, Event::PointerPressed { .. }) {
            ctx.keep_focus();
        }
        state.trigger.press.set_declines_focus(self.focusless);
        if !state.open {
            if state.trigger.press.update(event, self.is_active(), ctx) {
                self.open(state, ctx);
                return;
            }
            self.trigger.walk(event, &self.on_select, ctx);
            return;
        }

        if let Event::FocusLost = event {
            self.close(state, ctx);
            return;
        }
        let showing = self.showing(state);
        match self.dispatch(state, event, ctx) {
            Act::Kept => {}
            Act::Paint => ctx.request_paint(),
            Act::Tab(tab) => self.tab_to(state, tab, ctx),
            Act::Choose(row) => self.choose(state, showing, row, ctx),
            Act::Close => self.close(state, ctx),
        }
    }
}
