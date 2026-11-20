use rikka_app::prelude::*;
use rikka_ui::{Accept, Action};

use crate::widgets::row_drag;
use browser_model::Row;

const LINE: f32 = 2.0;
const RADIUS: f32 = Corner::Item.radius();

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DropLayout {
    Rows { pitch: f32, folders: Vec<bool> },
    Grid { columns: usize, pitch: Size },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DropTarget {
    pub(crate) index: usize,
    pub(crate) into: bool,
}

pub(crate) struct DropList {
    layout: DropLayout,
    count: usize,
    on_drop: Action<(Row, DropTarget)>,
    style: Style,
}

pub(crate) struct DropListView {
    list: DropList,
    content: Element,
}

pub(crate) fn drop_list(layout: DropLayout, count: usize, content: impl View) -> DropListView {
    DropListView {
        list: DropList {
            layout,
            count,
            on_drop: Action::default(),
            style: Style::default(),
        },
        content: content.build(),
    }
}

impl DropListView {
    pub(crate) fn on_drop<M>(
        mut self,
        on_drop: impl IntoHandler<(Row, DropTarget), (), M>,
    ) -> Self {
        self.list.on_drop = Action::new(on_drop);
        self
    }
}

impl Styled for DropListView {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.list.style
    }
}

impl View for DropListView {
    fn build(self) -> Element {
        Element::with_children(self.list, vec![self.content])
    }
}

#[derive(Debug, Default)]
pub(crate) struct DropListState {
    hovering: Option<DropTarget>,
}

impl DropList {
    fn target(&self, bounds: Rect, at: Point) -> DropTarget {
        let (x, y) = (at.x - bounds.x(), at.y - bounds.y());
        match &self.layout {
            DropLayout::Rows { pitch, folders } => {
                let reach = (y / pitch).max(0.0);
                let row = reach.floor() as usize;
                let within = reach.fract();
                if folders.get(row).copied().unwrap_or(false) && (0.25..0.75).contains(&within) {
                    return DropTarget {
                        index: row,
                        into: true,
                    };
                }
                DropTarget {
                    index: (reach.round() as usize).min(self.count),
                    into: false,
                }
            }
            DropLayout::Grid { columns, pitch } => {
                let columns = (*columns).max(1);
                let row = (y / pitch.height).max(0.0).floor() as usize;
                let column = (x / pitch.width).max(0.0).round() as usize;
                DropTarget {
                    index: (row * columns + column.min(columns)).min(self.count),
                    into: false,
                }
            }
        }
    }

    fn mark(&self, bounds: Rect, target: DropTarget) -> Rect {
        match &self.layout {
            DropLayout::Rows { pitch, .. } if target.into => Rect::new(
                bounds.x(),
                bounds.y() + target.index as f32 * pitch,
                bounds.width(),
                *pitch,
            ),
            DropLayout::Rows { pitch, .. } => Rect::new(
                bounds.x(),
                bounds.y() + target.index as f32 * pitch - LINE / 2.0,
                bounds.width(),
                LINE,
            ),
            DropLayout::Grid { columns, pitch } => {
                let columns = (*columns).max(1);
                let (row, column) = (target.index / columns, target.index % columns);
                Rect::new(
                    bounds.x() + column as f32 * pitch.width - LINE / 2.0,
                    bounds.y() + row as f32 * pitch.height,
                    LINE,
                    pitch.height,
                )
            }
        }
    }
}

impl Widget for DropList {
    type State = DropListState;

    fn layout(&self) -> Layout {
        Layout::column().with(&self.style)
    }

    fn behavior(&self, _state: &DropListState) -> Behavior {
        Behavior::CATCHES_DROPS
    }

    fn paint(&self, state: &mut DropListState, ctx: &mut PaintCtx<'_>) {
        let Some(target) = state.hovering else {
            return;
        };
        let accent = ctx.theme().palette().accent;
        let mark = self.mark(ctx.bounds(), target);
        let fill = match target.into {
            true => accent.with_alpha(0.16),
            false => accent,
        };
        ctx.draw_quad(Quad::new(mark).fill(fill).radius(match target.into {
            true => RADIUS,
            false => LINE / 2.0,
        }));
    }

    fn event(&self, state: &mut DropListState, event: &Event, ctx: &mut EventCtx<'_>) {
        match event {
            Event::DragMoved { position, offer } if row_drag::offered(offer) => {
                let target = self.target(ctx.bounds(), *position);
                ctx.accept_drag(Accept::moving(row_drag::mime()));
                ctx.set_handled();
                if state.hovering != Some(target) {
                    state.hovering = Some(target);
                    ctx.request_paint();
                }
            }
            Event::DragLeft => {
                if state.hovering.take().is_some() {
                    ctx.request_paint();
                }
            }
            Event::DragDropped {
                position, offer, ..
            } => {
                state.hovering = None;
                ctx.request_paint();
                let Some(row) = row_drag::read(offer) else {
                    return;
                };
                let target = self.target(ctx.bounds(), *position);
                ctx.set_handled();
                ctx.act(&self.on_drop, (row, target));
            }
            _ => {}
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.layout == other.layout
            && self.count == other.count
            && self.on_drop == other.on_drop
            && self.style == other.style
    }

    fn debug_name(&self) -> &'static str {
        "DropList"
    }
}
