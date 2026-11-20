use std::fs::File;
use std::io::Write;
use std::time::Duration;

use rikka_clipboard::Offer;
use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_protocols::wl_data_device_manager::DndAction;
use rikka_compositor_selection::{
    CompositorDragEvent, CompositorSource, SelectionEvent, SendDrag, cancel_compositor_drag,
    drop_compositor_drag, move_compositor_drag, request_drag_data, start_compositor_drag,
};
use rikka_compositor_surface::SurfaceId;
use rikka_types::{DragAction, DragActions};
use rikka_ui::DragOffer;

use super::state::State;
use super::transfer;
use crate::command::Command;
use crate::event::Event;
use crate::look::OutgoingDrag;
use crate::view::ViewId;

#[derive(Default)]
pub(crate) struct Drags {
    incoming: Option<Incoming>,
    delivering: Vec<(CompositorSource, DragOffer)>,
    outgoing: Option<Outgoing>,
    generation: u64,
}

struct Incoming {
    view: ViewId,
    source: CompositorSource,
}

struct Outgoing {
    view: ViewId,
    icon: Option<SurfaceId>,
    generation: u64,
}

fn wire_actions(actions: DragActions) -> DndAction {
    let mut wire = DndAction::empty();
    if actions.contains(DragAction::Copy) {
        wire |= DndAction::COPY;
    }
    if actions.contains(DragAction::Move) {
        wire |= DndAction::MOVE;
    }
    if actions.contains(DragAction::Ask) {
        wire |= DndAction::ASK;
    }
    wire
}

impl State {
    pub(crate) fn drop_moved(
        &mut self,
        view: ViewId,
        at: Point<f64, Logical>,
        offer: DragOffer,
        time: Duration,
    ) {
        let seat = self.seat;
        if self
            .drags
            .incoming
            .as_ref()
            .is_some_and(|incoming| incoming.view != view)
        {
            self.drop_left();
        }
        if self.drags.incoming.is_none() {
            let mimes = offer
                .mimes()
                .iter()
                .map(|mime| mime.clone().into_boxed_str())
                .collect();
            let actions = wire_actions(offer.source_actions());
            match start_compositor_drag(self, seat, mimes, actions, at) {
                Ok(source) => {
                    self.drags.incoming = Some(Incoming { view, source });
                    self.drags.delivering.push((source, offer));
                }
                Err(error) => {
                    tracing::debug!("failed to start drop into nested view: {error}");
                    return;
                }
            }
        }
        let under = self.surface_at(view, at);
        move_compositor_drag(self, seat, at, under, time);
    }

    pub(crate) fn drop_left(&mut self) {
        let Some(incoming) = self.drags.incoming.take() else {
            return;
        };
        let seat = self.seat;
        cancel_compositor_drag(self, seat);
        self.drags
            .delivering
            .retain(|(source, _)| *source != incoming.source);
        self.change_look(incoming.view, |look| look.drop_accepted = false);
    }

    pub(crate) fn drop_released(&mut self) {
        let Some(incoming) = self.drags.incoming.take() else {
            return;
        };
        let seat = self.seat;
        drop_compositor_drag(self, seat);
        self.change_look(incoming.view, |look| look.drop_accepted = false);
    }

    pub(crate) fn send_dropped(&mut self, request: SendDrag) {
        let Some(offer) = self
            .drags
            .delivering
            .iter()
            .find(|(source, _)| *source == request.source)
            .map(|(_, offer)| offer.clone())
        else {
            return;
        };
        let spawned = std::thread::Builder::new()
            .name("rikka-nested-drop".into())
            .spawn(move || {
                let Some(data) = offer.read(&request.mime_type) else {
                    return;
                };
                if let Err(error) = File::from(request.fd).write_all(&data) {
                    tracing::debug!("nested client closed drop pipe: {error}");
                }
            });
        if let Err(error) = spawned {
            tracing::warn!("failed to deliver drop: {error}");
        }
    }

    pub(crate) fn drag_event_arrived(&mut self, event: SelectionEvent) {
        match event {
            SelectionEvent::DragStarted { seat } if seat == self.seat => self.drag_started(),
            SelectionEvent::DragIconChanged { seat } if seat == self.seat => {
                let icon = self.selections.drag(seat).and_then(|drag| drag.icon);
                if let Some(outgoing) = self.drags.outgoing.as_mut() {
                    outgoing.icon = icon;
                }
                if let Some(icon) = icon {
                    self.drag_icon_committed(icon);
                }
            }
            SelectionEvent::DragEnded { seat, dropped } if seat == self.seat => {
                if let Some(outgoing) = self.drags.outgoing.take() {
                    self.change_look(outgoing.view, |look| look.drag = None);
                    self.event_sender.send(Event::DragEnded {
                        view: outgoing.view,
                        dropped,
                    });
                }
            }
            SelectionEvent::CompositorDrag { source, event, .. } => {
                self.compositor_drag_event(source, event)
            }
            _ => {}
        }
    }

    fn compositor_drag_event(&mut self, source: CompositorSource, event: CompositorDragEvent) {
        match event {
            CompositorDragEvent::Target(accepted) => {
                if let Some(view) = self
                    .drags
                    .incoming
                    .as_ref()
                    .filter(|incoming| incoming.source == source)
                    .map(|incoming| incoming.view)
                {
                    self.change_look(view, |look| look.drop_accepted = accepted);
                }
            }
            CompositorDragEvent::Finished(_) | CompositorDragEvent::Cancelled => {
                self.drags.delivering.retain(|(known, _)| *known != source);
            }
            CompositorDragEvent::Action(_) => {}
        }
    }

    fn drag_started(&mut self) {
        let seat = self.seat;
        let Some(drag) = self.selections.drag(seat) else {
            return;
        };
        if drag.driven {
            return;
        }
        let Some(view) = self.pointer.view.or(self.keyboard.focused) else {
            return;
        };
        let mimes: Vec<String> = self
            .selections
            .drag_mime_types(seat)
            .map(|mimes| mimes.iter().map(|mime| mime.to_string()).collect())
            .unwrap_or_default();
        self.drags.generation += 1;
        let generation = self.drags.generation;
        self.drags.outgoing = Some(Outgoing {
            view,
            icon: drag.icon,
            generation,
        });
        self.change_look(view, |look| {
            look.drag = Some(OutgoingDrag {
                offer: None,
                icon: None,
            });
        });
        if let Some(icon) = drag.icon {
            self.drag_icon_committed(icon);
        }
        self.event_sender.send(Event::DragStarted {
            view,
            mime_types: mimes.clone(),
        });
        let mut pending = Vec::with_capacity(mimes.len());
        for mime in &mimes {
            let Some((reading, write)) = transfer::pipe(mime) else {
                continue;
            };
            if request_drag_data(self, seat, mime, write).is_ok() {
                pending.push(reading);
            }
        }
        let link = self.link.clone();
        let spawned = std::thread::Builder::new()
            .name("rikka-nested-drag".into())
            .spawn(move || {
                let offer = transfer::gather(pending);
                let _ = link.send(Command::DragRead { generation, offer });
            });
        if let Err(error) = spawned {
            tracing::warn!("failed to read drag data: {error}");
        }
    }

    pub(crate) fn drag_read(&mut self, generation: u64, offer: Offer) {
        let Some(view) = self
            .drags
            .outgoing
            .as_ref()
            .filter(|outgoing| outgoing.generation == generation)
            .map(|outgoing| outgoing.view)
        else {
            return;
        };
        self.change_look(view, |look| {
            if let Some(drag) = look.drag.as_mut() {
                drag.offer = Some(offer);
            }
        });
    }

    pub(crate) fn drag_icon_committed(&mut self, root: SurfaceId) {
        let Some((view, icon)) = self
            .drags
            .outgoing
            .as_ref()
            .and_then(|outgoing| Some((outgoing.view, outgoing.icon?)))
        else {
            return;
        };
        if icon != root {
            return;
        }
        let layer = self.layer(icon, Point::zero());
        self.change_look(view, |look| {
            if let Some(drag) = look.drag.as_mut() {
                drag.icon = layer;
            }
        });
    }
}
