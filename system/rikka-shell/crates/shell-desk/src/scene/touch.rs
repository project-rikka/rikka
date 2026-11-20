use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use desk_layout::WidgetId;
use rikka_types::{Point, PointerButton};
use rikka_ui::{Command, DRAG_SLOP, Event, EventCtx, after, skip_view};
use shell_kit::Placement;

use crate::desk_scene::DeskScene;

const LONG_PRESS: Duration = Duration::from_millis(500);

#[derive(Clone, Debug)]
pub(super) struct Touch {
    pub(super) desk_scene: DeskScene,
    pub(super) screen: Placement,
}

#[derive(Debug, Default)]
pub(super) struct Pressing {
    from: Option<Point>,
    press: Rc<Cell<u64>>,
}

impl Pressing {
    pub(super) fn handle_touch(
        &mut self,
        touch: &Touch,
        widget: Option<WidgetId>,
        event: &Event,
        ctx: &mut EventCtx<'_>,
    ) {
        match event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } if ctx.bounds().contains(*position) => {
                self.hold(&touch.desk_scene, widget, *position);
                ctx.capture_pointer();
                ctx.set_handled();
            }
            Event::PointerPressed {
                button: PointerButton::Secondary,
                position,
                ..
            } if ctx.bounds().contains(*position) => {
                self.forget();
                touch
                    .desk_scene
                    .offer_menu_at(widget, &touch.screen, *position);
                ctx.set_handled();
            }
            Event::PointerMoved(position) => {
                if self
                    .from
                    .is_some_and(|from| from.distance(*position) > DRAG_SLOP)
                {
                    self.forget();
                    ctx.release_pointer();
                }
            }
            Event::PointerReleased { .. } if self.from.is_some() => {
                self.forget();
                ctx.release_pointer();
                ctx.set_handled();
            }
            Event::Command(Command::ContextMenu) => {
                touch.desk_scene.offer_menu(widget);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn hold(&mut self, touch: &DeskScene, widget: Option<WidgetId>, at: Point) {
        self.from = Some(at);
        let press = self.press.get().wrapping_add(1);
        self.press.set(press);
        let (press_counter, touch) = (Rc::clone(&self.press), touch.clone());
        after(LONG_PRESS, move || {
            if press_counter.get() == press {
                press_counter.set(press.wrapping_add(1));
                touch.edit(widget);
            }
            skip_view();
        });
    }

    fn forget(&mut self) {
        self.from = None;
        self.press.set(self.press.get().wrapping_add(1));
    }
}
