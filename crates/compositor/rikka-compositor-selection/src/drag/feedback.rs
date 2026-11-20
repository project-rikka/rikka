use rikka_compositor_protocols::wl_data_device_manager::DndAction;
use rikka_compositor_protocols::wl_data_source::WlDataSource;
use rikka_compositor_seat::SeatId;

use crate::access::Access;
use crate::event::{CompositorDragEvent, SelectionEvent};
use crate::ids::{CompositorSource, SourceId};
use crate::source::{ACTIONS_SINCE, Origin};

#[derive(Debug, Clone)]
pub(crate) enum Feedback {
    Client(WlDataSource),
    Compositor {
        seat: SeatId,
        source: CompositorSource,
    },
}

impl Feedback {
    pub(crate) fn for_source<A: Access>(state: &mut A, source: SourceId) -> Option<Self> {
        match &state.selections().sources.get(source)?.origin {
            Origin::Data(data) => Some(Self::Client(data.clone())),
            Origin::Dragged { seat } => Some(Self::Compositor {
                seat: *seat,
                source: CompositorSource(source),
            }),
            _ => None,
        }
    }

    pub(crate) fn target<A: Access>(&self, state: &mut A, mime_type: Option<&str>) {
        match self {
            Self::Client(source) => source.target(mime_type),
            Self::Compositor { .. } => {
                self.emit(state, CompositorDragEvent::Target(mime_type.is_some()))
            }
        }
    }

    pub(crate) fn action<A: Access>(&self, state: &mut A, action: DndAction) {
        match self {
            Self::Client(source) => source.action(action),
            Self::Compositor { .. } => self.emit(state, CompositorDragEvent::Action(action)),
        }
    }

    pub(crate) fn performed<A: Access>(&self, _state: &mut A) {
        if let Self::Client(source) = self
            && source.version() >= ACTIONS_SINCE
        {
            source.dnd_drop_performed();
        }
    }

    pub(crate) fn finished<A: Access>(&self, state: &mut A, resolved: Option<DndAction>) {
        match self {
            Self::Client(source) => {
                if let Some(action) = resolved {
                    source.action(action);
                }
                source.dnd_finished();
            }
            Self::Compositor { .. } => self.emit(state, CompositorDragEvent::Finished(resolved)),
        }
    }

    pub(crate) fn cancelled<A: Access>(&self, state: &mut A) {
        match self {
            Self::Client(source) => {
                if source.version() >= ACTIONS_SINCE {
                    source.cancelled();
                }
            }
            Self::Compositor { .. } => self.emit(state, CompositorDragEvent::Cancelled),
        }
    }

    fn emit<A: Access>(&self, state: &mut A, event: CompositorDragEvent) {
        if let Self::Compositor { seat, source } = *self {
            state.report(SelectionEvent::CompositorDrag {
                seat,
                source,
                event,
            });
        }
    }
}
