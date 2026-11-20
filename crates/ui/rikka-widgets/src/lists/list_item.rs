use std::hash::Hash;
use std::time::Instant;

use rikka_symbols::MaterialSymbol;
use rikka_text::IconSpec;
use rikka_types::{Color, Key, PointerButton, Rect};
use rikka_ui::text::{Label, TextRun};
use rikka_ui::{
    Action, Behavior, Element, Event, EventCtx, Handler, IconSource, IntoHandler, Layout, Length,
    PaintCtx, Widget, hash_of,
};

use crate::control::ControlSize;
use crate::controls::checkbox::CheckState;
use crate::controls::mark::Mark;
use crate::controls::{choice, spinner};
use crate::rows::{self, draw_cell};

const TWO_LINE: f32 = 1.5;
const HEADER_HEIGHT: f32 = 0.85;
const HEADER_FONT: f32 = 0.85;
const SECONDARY_FONT: f32 = 0.86;
const CHEVRON_GAP: f32 = 2.0;
const CHEVRON_TARGET: f32 = 4.0;
const ACTION_TARGET: f32 = 4.0;
const CHECK_TARGET: f32 = 5.0;
const TAG_GAP: f32 = 0.5;

#[derive(Clone, Debug, PartialEq)]
struct Tag {
    tag_label: Label,
    color: Color,
}

#[must_use]
#[derive(Clone)]
pub struct ListItem {
    label: Label,
    secondary: Option<Label>,
    trailing: Option<Label>,
    tags: Vec<Tag>,
    action: Option<MaterialSymbol>,
    check: Option<CheckState>,
    icon: Option<IconSource>,
    busy: bool,
    tint: Option<Color>,
    ink: Option<Color>,
    depth: usize,
    expanded: Option<bool>,
    enabled: bool,
    header: bool,
    key: Option<u64>,
    on_check: Action<(bool,)>,
    on_expand: Action,
    on_action: Action,
}

pub fn list_item(label: impl Into<Label>) -> ListItem {
    ListItem {
        label: label.into(),
        secondary: None,
        trailing: None,
        tags: Vec::new(),
        action: None,
        check: None,
        icon: None,
        busy: false,
        tint: None,
        ink: None,
        depth: 0,
        expanded: None,
        enabled: true,
        header: false,
        key: None,
        on_check: Action::default(),
        on_expand: Action::default(),
        on_action: Action::default(),
    }
}

pub fn list_header(label: impl Into<Label>) -> ListItem {
    ListItem {
        header: true,
        ..list_item(label)
    }
}

impl ListItem {
    pub fn secondary(mut self, secondary: impl Into<Label>) -> Self {
        self.secondary = Some(secondary.into());
        self
    }

    pub fn trailing(mut self, trailing: impl Into<Label>) -> Self {
        self.trailing = Some(trailing.into());
        self
    }

    pub fn tag(mut self, tag_label: impl Into<Label>, color: Color) -> Self {
        self.tags.push(Tag {
            tag_label: tag_label.into(),
            color,
        });
        self
    }

    pub fn check(mut self, state: impl Into<CheckState>) -> Self {
        self.check = Some(state.into());
        self
    }

    pub fn icon(mut self, icon: impl Into<IconSource>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    pub fn tint(mut self, color: Color) -> Self {
        self.tint = Some(color);
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.ink = Some(color);
        self
    }

    pub fn depth(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn keyed(mut self, key: impl Hash) -> Self {
        self.key = Some(hash_of(key));
        self
    }

    pub fn on_check<Marker>(mut self, on_check: impl IntoHandler<(bool,), (), Marker>) -> Self {
        self.on_check = Action::new(on_check);
        self
    }

    pub fn on_expand<Marker>(mut self, on_expand: impl IntoHandler<(), (), Marker>) -> Self {
        self.on_expand = Action::new(on_expand);
        self
    }

    pub fn on_action<Marker>(
        mut self,
        symbol: MaterialSymbol,
        on_action: impl IntoHandler<(), (), Marker>,
    ) -> Self {
        self.action = Some(symbol);
        self.on_action = Action::new(on_action);
        self
    }

    pub(crate) fn name(&self) -> u64 {
        match self.key {
            Some(key) => key,
            None => hash_of((&self.label, &self.secondary, self.depth)),
        }
    }

    pub(crate) fn commanded(&self, key: Key) -> Option<Handler> {
        match key {
            Key::ArrowLeft | Key::ArrowRight => {
                let open = key == Key::ArrowRight;
                let toggle = self.on_expand.handler()?;
                (self.expanded == Some(!open)).then(|| toggle.clone())
            }
            Key::Space if self.check.is_some() && self.enabled => {
                let checked = self.check.unwrap_or_default().toggled();
                let check = self.on_check.handler()?.clone();
                Some(check.map(move |()| (checked,)))
            }
            _ => None,
        }
    }
    pub(crate) fn selectable(&self) -> bool {
        !self.header && self.enabled
    }

    pub(crate) fn is_header(&self) -> bool {
        self.header
    }

    fn leads(&self) -> bool {
        self.icon.is_some() || self.busy
    }

    fn looks_like(&self, other: &ListItem) -> bool {
        self.label == other.label
            && self.secondary == other.secondary
            && self.trailing == other.trailing
            && self.tags == other.tags
            && self.action == other.action
            && self.check == other.check
            && self.icon == other.icon
            && self.busy == other.busy
            && self.tint == other.tint
            && self.ink == other.ink
            && self.depth == other.depth
            && self.expanded == other.expanded
            && self.enabled == other.enabled
            && self.header == other.header
            && self.key == other.key
            && self.on_check == other.on_check
            && self.on_expand == other.on_expand
            && self.on_action == other.on_action
    }
}

impl std::fmt::Debug for ListItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListItem")
            .field("label", &self.label)
            .field("depth", &self.depth)
            .field("header", &self.header)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Metrics {
    pub(crate) size: ControlSize,
    two_line: bool,
    checks: bool,
    chevrons: bool,
}

impl Metrics {
    pub(crate) fn measure(items: &[ListItem], size: ControlSize) -> Self {
        let mut metrics = Self {
            size,
            ..Self::default()
        };
        for item in items {
            metrics.two_line |= item.secondary.is_some();
            metrics.chevrons |= item.expanded.is_some();
            metrics.checks |= item.check.is_some();
        }
        metrics
    }

    pub(crate) fn row_height(&self) -> f32 {
        self.size.height() * if self.two_line { TWO_LINE } else { 1.0 }
    }

    pub(crate) fn header_height(&self) -> f32 {
        self.size.height() * HEADER_HEIGHT
    }

    fn height(&self, item: &ListItem) -> f32 {
        match item.header {
            true => self.header_height(),
            false => self.row_height(),
        }
    }

    fn indent(&self) -> f32 {
        self.size.icon_size()
    }

    fn check_x(&self, row: Rect) -> f32 {
        row.x() + choice::ROW_PADDING
    }

    fn chevron_x(&self, row: Rect, item: &ListItem) -> f32 {
        let gutter = match self.checks {
            true => self.size.indicator() + self.size.gap(),
            false => 0.0,
        };
        self.check_x(row) + gutter + item.depth as f32 * self.indent()
    }

    fn icon_x(&self, row: Rect, item: &ListItem) -> f32 {
        let gutter = match self.chevrons {
            true => self.size.icon_size() + CHEVRON_GAP,
            false => 0.0,
        };
        self.chevron_x(row, item) + gutter
    }

    fn content_x(&self, row: Rect, item: &ListItem) -> f32 {
        let gutter = match item.leads() {
            true => self.size.icon_size() + self.size.gap(),
            false => 0.0,
        };
        self.icon_x(row, item) + gutter
    }

    fn glyph_rect(&self, x: f32, row: Rect) -> Rect {
        rows::glyph_rect(x, row.center().y, self.size.icon_size())
    }

    fn check_box(&self, row: Rect) -> Rect {
        let side = self.size.indicator();
        Rect::new(self.check_x(row), row.center().y - side / 2.0, side, side)
    }

    fn check_rect(&self, row: Rect, item: &ListItem) -> Option<Rect> {
        item.check?;
        item.on_check
            .handler()
            .map(|_| self.check_box(row).inflate(CHECK_TARGET))
    }

    fn chevron_rect(&self, row: Rect, item: &ListItem) -> Option<Rect> {
        item.expanded?;
        let size = self.size.icon_size();
        let x = self.chevron_x(row, item);
        Some(Rect::new(x, row.center().y - size / 2.0, size, size).inflate(CHEVRON_TARGET))
    }

    fn action_glyph_rect(&self, row: Rect) -> Rect {
        let size = self.size.icon_size();
        rows::glyph_rect(row.max_x() - self.size.gap() - size, row.center().y, size)
    }

    fn action_rect(&self, row: Rect, item: &ListItem) -> Option<Rect> {
        item.action?;
        item.on_action
            .handler()
            .map(|_| self.action_glyph_rect(row).inflate(ACTION_TARGET))
    }
}

pub(crate) struct Row {
    item: ListItem,
    metrics: Metrics,
    selected: bool,
}

pub(crate) fn row(item: ListItem, metrics: Metrics, selected: bool) -> Element {
    let name = item.name();
    Element::new(Row {
        item,
        metrics,
        selected,
    })
    .keyed(name)
}

#[derive(Debug, Default)]
pub(crate) struct RowState {
    hovered: bool,
    hot_action: bool,
    spun: Option<Instant>,
}

impl Row {
    fn shows_action(&self, state: &RowState) -> bool {
        self.item.on_action.is_set() && (state.hovered || self.selected)
    }

    fn paint_action(&self, state: &RowState, ctx: &mut PaintCtx<'_>, row: Rect) -> Option<Rect> {
        if !self.shows_action(state) {
            return None;
        }
        let glyph = self.item.action?;
        let target = self.metrics.action_rect(row, &self.item)?;
        let palette = ctx.theme().palette();
        if state.hot_action {
            ctx.draw_circle(target.center(), target.width(), ctx.theme().hover_layer());
        }
        let color = match state.hot_action {
            true => palette.text,
            false => palette.text_muted,
        };
        ctx.draw_icon(
            self.metrics.action_glyph_rect(row),
            IconSpec::new(glyph.codepoint(), self.metrics.size.icon_size()),
            color,
        );
        Some(target)
    }

    fn paint_ends(&self, ctx: &mut PaintCtx<'_>, row: Rect, x: f32, meta_color: Color) -> f32 {
        let inset = self.metrics.size.gap();
        let size = self.metrics.size.font_size(ctx.theme()) * SECONDARY_FONT;
        let mut edge = row.max_x();
        for (at, tag) in self.item.tags.iter().rev().enumerate() {
            let gap = match at {
                0 => inset,
                _ => inset * TAG_GAP,
            };
            let run = TextRun::new(tag.tag_label.clone(), size).medium().no_wrap();
            let color = ctx.theme().dim(tag.color, !self.item.enabled);
            let room = Rect::new(x, row.y(), (edge - gap - x).max(0.0), row.height());
            match rows::draw_tag(ctx, room, run, color) {
                Some(start) => edge = start,
                None => break,
            }
        }

        let before = Rect::new(row.x(), row.y(), (edge - row.x()).max(0.0), row.height());
        let width = (before.max_x() - inset - x).max(0.0);
        match &self.item.trailing {
            Some(trailing) => {
                let run = TextRun::new(trailing.clone(), size).no_wrap();
                rows::draw_trailing(ctx, before, inset, width, run, meta_color)
            }
            None => width,
        }
    }

    fn paint_header(&self, ctx: &mut PaintCtx<'_>, row: Rect) {
        let palette = ctx.theme().palette();
        let inset = self.metrics.size.gap();
        let font_size = self.metrics.size.font_size(ctx.theme());
        let run = TextRun::new(self.item.label.clone(), font_size * HEADER_FONT)
            .medium()
            .no_wrap();
        let rect = Rect::new(
            row.x() + inset,
            row.max_y() - run.line_height,
            (row.width() - inset * 2.0).max(0.0),
            run.line_height,
        );
        draw_cell(ctx, rect, rect.x(), rect.width(), run, palette.text_muted);
    }
}

impl Widget for Row {
    type State = RowState;

    fn layout(&self) -> Layout {
        Layout::row()
            .size(Length::FILL, Length::px(self.metrics.height(&self.item)))
            .fixed()
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::TRACKS_HOVER
    }

    fn paint(&self, state: &mut RowState, ctx: &mut PaintCtx<'_>) {
        let row = ctx.bounds();
        if self.item.header {
            self.paint_header(ctx, row);
            return;
        }

        let palette = ctx.theme().palette();
        let theme_dim =
            |color: Color, ctx: &PaintCtx<'_>| ctx.theme().dim(color, !self.item.enabled);
        let ink = match self.selected {
            true => palette.on_selection,
            false => self.item.ink.unwrap_or(palette.text),
        };
        let label_color = theme_dim(ink, ctx);
        let meta_color = theme_dim(palette.text_muted, ctx);
        let font_size = self.metrics.size.font_size(ctx.theme());
        let icon_size = self.metrics.size.icon_size();

        if let Some(check) = self.item.check {
            let box_rect = self.metrics.check_box(row);
            let mark = Mark::settled(check, ctx.theme(), box_rect.width(), self.item.enabled);
            mark.paint(ctx, box_rect);
        }
        if let Some(expanded) = self.item.expanded {
            let glyph = match expanded {
                true => MaterialSymbol::EXPAND_MORE,
                false => MaterialSymbol::CHEVRON_RIGHT,
            };
            ctx.draw_icon(
                self.metrics
                    .glyph_rect(self.metrics.chevron_x(row, &self.item), row),
                IconSpec::new(glyph.codepoint(), icon_size),
                meta_color,
            );
        }
        if self.item.busy {
            let tint = self.item.tint.unwrap_or(palette.accent);
            let track = meta_color.scale_alpha(spinner::TRACK_ALPHA);
            let tint = theme_dim(tint, ctx);
            spinner::spun(
                ctx,
                &mut state.spun,
                self.metrics
                    .glyph_rect(self.metrics.icon_x(row, &self.item), row),
                tint,
                track,
            );
        } else if let Some(icon) = &self.item.icon {
            let tint = self
                .item
                .tint
                .map_or(meta_color, |tint| theme_dim(tint, ctx));
            ctx.draw_icon_source(
                self.metrics
                    .glyph_rect(self.metrics.icon_x(row, &self.item), row),
                icon,
                icon_size,
                tint,
            );
        }

        let x = self.metrics.content_x(row, &self.item);
        let width = match self.paint_action(state, ctx, row) {
            Some(occupied) => (occupied.min_x() - x).max(0.0),
            None => self.paint_ends(ctx, row, x, meta_color),
        };

        let label = TextRun::new(self.item.label.clone(), font_size).no_wrap();
        match &self.item.secondary {
            None => draw_cell(ctx, row, x, width, label, label_color),
            Some(secondary) => {
                let secondary =
                    TextRun::new(secondary.clone(), font_size * SECONDARY_FONT).no_wrap();
                let top = row.center().y - (label.line_height + secondary.line_height) / 2.0;
                let first = Rect::new(x, top, width, label.line_height);
                let second = Rect::new(x, top + label.line_height, width, secondary.line_height);
                draw_cell(ctx, first, x, width, label, label_color);
                draw_cell(ctx, second, x, width, secondary, meta_color);
            }
        }
    }

    fn event(&self, state: &mut RowState, event: &Event, ctx: &mut EventCtx<'_>) {
        match event {
            Event::PointerMoved(position) => {
                let hot = self
                    .metrics
                    .action_rect(ctx.bounds(), &self.item)
                    .is_some_and(|target| target.contains(*position));
                if !state.hovered || state.hot_action != hot {
                    state.hovered = true;
                    state.hot_action = hot;
                    ctx.request_paint();
                }
            }
            Event::PointerEntered => {
                state.hovered = true;
                ctx.request_paint();
            }
            Event::PointerExited => {
                state.hovered = false;
                state.hot_action = false;
                ctx.request_paint();
            }
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                let row = ctx.bounds();
                if let Some(target) = self.metrics.check_rect(row, &self.item)
                    && target.contains(*position)
                    && self.item.on_check.active(self.item.enabled)
                {
                    ctx.set_handled();
                    ctx.act(
                        &self.item.on_check,
                        (self.item.check.unwrap_or_default().toggled(),),
                    );
                    return;
                }
                if let Some(target) = self.metrics.chevron_rect(row, &self.item)
                    && target.contains(*position)
                    && self.item.on_expand.is_set()
                {
                    ctx.set_handled();
                    ctx.act(&self.item.on_expand, ());
                    return;
                }
                if let Some(target) = self.metrics.action_rect(row, &self.item)
                    && target.contains(*position)
                    && self.item.on_action.is_set()
                {
                    ctx.set_handled();
                    ctx.act(&self.item.on_action, ());
                }
            }
            _ => {}
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.item.looks_like(&other.item)
            && self.metrics == other.metrics
            && self.selected == other.selected
    }

    fn debug_name(&self) -> &'static str {
        "ListRow"
    }
}

impl std::fmt::Debug for Row {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListRow")
            .field("label", &self.item.label)
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}

pub(crate) struct EmptyRow;

impl Widget for EmptyRow {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::px(0.0)).fixed()
    }

    fn paint(&self, (): &mut (), _ctx: &mut PaintCtx<'_>) {}

    fn same(&self, _other: &Self) -> bool {
        true
    }

    fn debug_name(&self) -> &'static str {
        "MissingRow"
    }
}
