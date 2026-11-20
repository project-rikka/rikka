mod chrome;
mod guides;
mod marks;
mod toolbar;

use desk_layout::{Area, Layout, WidgetId};
use rikka_types::{CornerRadii, RoundedRect};
use rikka_ui::{Behavior, Element, Layout as Flow, Length, PaintCtx, View, Widget, theme};
use shell_kit::taskbar::Shape;

use crate::editor_canvas::card_radius;
use crate::session::Session;

pub(crate) struct Overlay {
    pub(crate) session: Session,
    pub(crate) area: Area,
    pub(crate) layout: Layout,
    pub(crate) selected: Option<WidgetId>,
    pub(crate) taskbar: Option<Shape>,
}

#[derive(Debug, Default)]
pub(crate) struct OverlayState;

impl Widget for Overlay {
    type State = OverlayState;

    fn layout(&self) -> Flow {
        Flow::column().size(Length::FILL, Length::FILL)
    }

    fn behavior(&self, _state: &OverlayState) -> Behavior {
        Behavior::INERT
    }

    fn paint(&self, _state: &mut OverlayState, ctx: &mut PaintCtx<'_>) {
        let Some(shot) = self.session.shot() else {
            return;
        };
        if shot.moving {
            ctx.request_frame();
        }
        let theme = theme().clone();
        let draft = self.session.draft();
        let marks = self.session.marks();
        let layout = draft.as_ref().map_or(&self.layout, |draft| &draft.layout);
        let grabbed = draft.as_ref().and_then(|draft| draft.grabbed);
        let card = RoundedRect::new(shot.card, CornerRadii::uniform(card_radius(shot.spread)));
        ctx.with_opacity(shot.spread, |ctx| {
            ctx.clipped(card, |ctx| {
                if let Some(shape) = self.taskbar {
                    chrome::taskbar(shot.card, shot.projection, shape, &theme, ctx);
                }
            });
        });
        if shot.spread <= 0.0 {
            self.session.lay_tools(Vec::new());
            return;
        }
        let shown = marks::Shown {
            layout,
            area: &self.area,
            projection: shot.projection,
            grabbed,
            theme: &theme,
        };
        ctx.with_opacity(shot.spread, |ctx| {
            if let Some(hovered) = marks.hovered.and_then(|hovered| shown.hovered(hovered))
                && Some(hovered) != self.selected
            {
                shown.hover_ring(hovered, ctx);
            }
            let tools = match self.selected {
                Some(id) => shown.selection(id, shot.card, marks.hovered, !shot.moving, ctx),
                None => Vec::new(),
            };
            self.session.lay_tools(tools);
            guides::draw(&marks, shot.projection, &theme, ctx);
            if let Some(drag_preview) = marks.drag_preview {
                ctx.topmost(|ctx| marks::drag_preview(drag_preview, shot.projection, &theme, ctx));
            }
        });
    }

    fn debug_name(&self) -> &'static str {
        "DeskOverlay"
    }
}

impl View for Overlay {
    fn build(self) -> Element {
        Element::new(self)
    }
}

impl std::fmt::Debug for Overlay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Overlay")
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}
