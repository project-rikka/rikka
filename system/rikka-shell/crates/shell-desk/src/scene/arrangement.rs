use desk_layout::Widget as Placed;
use rikka_ui::{Element, Event, EventCtx, Layout, Length, PaintCtx, View, Widget, component_with};

use crate::scene::Scene;
use crate::scene::seat::Seat;
use crate::scene::touch::{Pressing, Touch};
use crate::scene::transitions::{ExitProgress, WidgetTransitions};
use crate::widgets::{self, Live, Mode};

pub(super) struct Arrangement {
    pub(super) scene: Scene,
    pub(super) touch: Option<Touch>,
}

#[derive(Debug, Default)]
pub(super) struct ArrangementState {
    transitions: WidgetTransitions,
    pressing: Pressing,
}

#[derive(PartialEq)]
struct Showing {
    widget: Placed,
    mode: Mode,
}

impl Widget for Arrangement {
    type State = ArrangementState;

    fn layout(&self) -> Layout {
        let output = self.scene.output;
        Layout::DEFAULT.size(Length::px(output.width), Length::px(output.height))
    }

    fn build(&self, state: &mut ArrangementState) -> Vec<Element> {
        let scene = &self.scene;
        let transitions = &mut state.transitions;
        let transition_style = transitions.follow(
            scene.wallpaper.as_deref(),
            &scene.layout.widgets,
            scene.area,
        );
        let leaving = transitions.leaving().iter().map(|leaver| {
            let seat = Seat {
                widget: leaver.widget.id,
                rect: leaver.rect,
                transition_style: leaver.transition_style,
                leaving: true,
                grabbed: false,
                touch: None,
            };
            seated(seat, &leaver.widget, scene.mode, leaver.key)
        });
        let staying = scene.layout.widgets.iter().map(|widget| {
            let seat = Seat {
                widget: widget.id,
                rect: scene.area.place(widget.size, widget.spot),
                transition_style,
                leaving: false,
                grabbed: scene.grabbed == Some(widget.id),
                touch: self.touch.clone(),
            };
            seated(seat, widget, scene.mode, transitions.key(widget.id))
        });
        leaving.chain(staying).collect()
    }

    fn paint(&self, state: &mut ArrangementState, ctx: &mut PaintCtx<'_>) {
        match state.transitions.exit_progress(ctx.now()) {
            ExitProgress::Idle => {}
            ExitProgress::Finished => ctx.request_build(),
            ExitProgress::NextDeadline(at) => ctx.wake_at(at),
        }
    }

    fn event(&self, state: &mut ArrangementState, event: &Event, ctx: &mut EventCtx<'_>) {
        if let Some(touch) = &self.touch {
            state.pressing.handle_touch(touch, None, event, ctx);
        }
    }

    fn same(&self, other: &Arrangement) -> bool {
        let (scene, other_scene) = (&self.scene, &other.scene);
        scene.wallpaper == other_scene.wallpaper
            && scene.layout == other_scene.layout
            && scene.output == other_scene.output
            && scene.area == other_scene.area
            && scene.mode == other_scene.mode
            && scene.grabbed == other_scene.grabbed
            && self.touch.as_ref().map(|touch| &touch.screen)
                == other.touch.as_ref().map(|touch| &touch.screen)
    }
}

fn seated(seat: Seat, widget: &Placed, mode: Mode, key: u64) -> Element {
    let showing = Showing {
        widget: widget.clone(),
        mode,
    };
    seat.holding(component_with(live, showing, shown))
        .keyed(key)
}

fn live(live: &Live) -> &Live {
    live
}

fn shown(live: &Live, showing: &Showing) -> Element {
    widgets::widget(&showing.widget, live, showing.mode)
}

impl View for Arrangement {
    fn build(self) -> Element {
        Element::new(self)
    }
}

impl std::fmt::Debug for Arrangement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Arrangement")
            .field("wallpaper", &self.scene.wallpaper)
            .field("widgets", &self.scene.layout.widgets.len())
            .finish_non_exhaustive()
    }
}
