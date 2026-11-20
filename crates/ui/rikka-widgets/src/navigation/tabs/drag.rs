use rikka_theme::Theme;
use rikka_types::{Point, Rect, Size};
use rikka_ui::{
    Accept, DragActions, DragEnd, DragIcon, DragOffer, DragStart, DragWindow, EventCtx, Handler,
    Offer, View, has_drag_payload,
};

use crate::control::ControlSize;
use crate::navigation::tabs::strip::PADDING;
use crate::navigation::tabs::transfer::{self, Dragging, PendingDrag};
use crate::navigation::tabs::{Tab, Tabs, TabsState, tabs};

impl Tabs {
    pub(super) fn lift(
        &self,
        state: &mut TabsState,
        ctx: &mut EventCtx<'_>,
        bounds: Rect,
        armed: PendingDrag,
        position: Point,
    ) {
        state.armed = None;
        ctx.release_pointer();
        let (Some(mime), Some(tab)) = (self.mime(), self.tabs.get(armed.index)) else {
            return;
        };
        let strip = self.strip(state, bounds, state.scroll.rendered());
        let rect = strip.tab_rect(armed.index as f32);
        let icon = ghost(
            tab.clone(),
            self.step(),
            rect,
            position,
            ctx.theme().clone(),
        );
        let offer = Offer::new().with(mime, tab.label.as_str().as_bytes().to_vec());
        ctx.start_drag(
            DragStart::new(offer, icon)
                .actions(DragActions::MOVE)
                .moves_windows(true),
        );
        let follows = self.tabs.len() <= 1 && !self.moving.tears_off_last();
        if follows {
            ctx.carry_window(DragWindow::This { grip: armed.origin });
        }
        state.hot = None;
        state.dragging = Some(Dragging {
            index: armed.index,
            lifted: false,
            follows,
            grip: Point::new(strip.lane().x() + armed.origin.x - rect.x(), armed.origin.y),
        });
        state.landing = (!follows).then_some(armed.index);
        ctx.set_handled();
        ctx.request_paint();
    }

    pub(super) fn landing_for(
        &self,
        state: &TabsState,
        bounds: Rect,
        position: Point,
        offer: &DragOffer,
    ) -> Option<(String, usize)> {
        let mime = self.mime().filter(|mime| offer.offers(mime))?;
        let slots = self.slots(state, true);
        let strip = self.laid_out(state, bounds, state.scroll.rendered(), slots);
        Some((mime, transfer::landing_at(&strip, position, slots)))
    }

    pub(super) fn drag_over(
        &self,
        state: &mut TabsState,
        ctx: &mut EventCtx<'_>,
        bounds: Rect,
        position: Point,
        offer: &DragOffer,
    ) {
        let Some((mime, at)) = self.landing_for(state, bounds, position, offer) else {
            return;
        };
        if state.landing != Some(at) {
            state.landing = Some(at);
            ctx.request_paint();
        }
        ctx.accept_drag(Accept::moving(mime));
        ctx.set_handled();
    }

    pub(super) fn dropped(
        &self,
        state: &mut TabsState,
        ctx: &mut EventCtx<'_>,
        bounds: Rect,
        position: Point,
        offer: &DragOffer,
    ) {
        let Some((_, at)) = self.landing_for(state, bounds, position, offer) else {
            return;
        };
        state.close_gap();
        ctx.set_handled();
        ctx.request_paint();
        if let Some(land) = self.moving.land() {
            ctx.call(land, (at,));
        }
    }

    pub(super) fn lifted(&self, state: &mut TabsState, ctx: &mut EventCtx<'_>) {
        let Some(dragging) = state.dragging.as_mut().filter(|drag| !drag.lifted) else {
            return;
        };
        dragging.lifted = true;
        let index = dragging.index;
        if let Some(transfer) = self.moving.transfer() {
            ctx.call(&transfer.lift, (index,));
        }
        ctx.request_paint();
    }

    pub(super) fn went_outside(&self, state: &mut TabsState, ctx: &mut EventCtx<'_>) {
        let Some(dragging) = state.dragging.as_mut().filter(|drag| !drag.follows) else {
            return;
        };
        let Some((on_lift, on_tear_off)) = self.tearing_off() else {
            return;
        };
        if !dragging.lifted {
            dragging.lifted = true;
            ctx.call(on_lift, (dragging.index,));
            ctx.request_paint();
        }
        if has_drag_payload() {
            let grip = dragging.grip;
            ctx.call(on_tear_off, ());
            ctx.carry_window(DragWindow::Opening { grip });
        }
    }

    pub(super) fn reclaimed(&self, ctx: &mut EventCtx<'_>) {
        let Some(transfer) = self.moving.transfer() else {
            return;
        };
        if self.tabs.is_empty() {
            return;
        }
        ctx.set_handled();
        ctx.call(&transfer.lift, (0,));
        self.settled(ctx);
    }

    pub(super) fn drag_ended(&self, state: &mut TabsState, ctx: &mut EventCtx<'_>, end: DragEnd) {
        let Some(dragging) = state.dragging.take() else {
            return;
        };
        state.close_gap();
        state.hot = None;
        ctx.request_paint();
        if dragging.lifted {
            if has_drag_payload()
                && let Some(land) = self.moving.land()
            {
                ctx.call(land, (dragging.index,));
            }
            self.settled(ctx);
            return;
        }
        if end != DragEnd::Nowhere || dragging.follows {
            return;
        }
        let Some((on_lift, on_tear_off)) = self.tearing_off() else {
            return;
        };
        ctx.call(on_lift, (dragging.index,));
        ctx.call(on_tear_off, ());
        self.settled(ctx);
    }

    fn tearing_off(&self) -> Option<(&Handler<(usize,)>, &Handler)> {
        let transfer = self.moving.transfer()?;
        Some((&transfer.lift, transfer.tear_off.as_ref()?))
    }

    fn settled(&self, ctx: &mut EventCtx<'_>) {
        if let Some(on_settled) = self.on_settled.handler() {
            ctx.call(on_settled, ());
        }
    }
}

fn ghost(tab: Tab, step: ControlSize, rect: Rect, position: Point, theme: Theme) -> DragIcon {
    let size = Size::new(rect.width() + PADDING * 2.0, rect.height() + PADDING * 2.0);
    let hotspot = Point::new(
        position.x - rect.x() + PADDING,
        position.y - rect.y() + PADDING,
    );
    let carried = tab.closable(false);
    DragIcon::new(size, hotspot, theme, move || {
        tabs(0).items([carried.clone()]).control_size(step).build()
    })
}
