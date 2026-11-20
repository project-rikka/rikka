use rikka_compositor_output::{OutputHost, Outputs};
use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_seat::{SeatEvent, SeatHost, Seats};
use rikka_compositor_selection::{
    SelectionEvent, SelectionHost, SelectionKind, Selections, SendDrag, SendSelection,
};
use rikka_compositor_server::{Client, DisplayHandle};
use rikka_compositor_shell::{Shell, ShellEvent, ShellHost, TokenRequest};
use rikka_compositor_surface::{
    BufferChange, Changes, Committed, SurfaceHost, SurfaceId, Surfaces,
};
use rikka_compositor_types::OutputId;

use super::state::State;

const PACED: Changes = Changes::FRAME_CALLBACKS.union(Changes::FEEDBACK);

impl SurfaceHost for State {
    fn surfaces(&mut self) -> &mut Surfaces {
        &mut self.surfaces
    }

    fn committed(&mut self, commit: &Committed) {
        rikka_compositor_seat::surface_committed(self, commit);
        let drawn =
            commit.buffer != BufferChange::Unchanged || commit.changes.contains(Changes::DAMAGE);
        if drawn {
            self.contents.touch(commit.surface);
        }
        let root = self.surfaces.root(commit.surface);
        self.ornament_committed(root);
        let visual =
            commit.buffer != BufferChange::Unchanged || !(commit.changes - PACED).is_empty();
        if visual && let Some(view) = self.view_of(commit.surface) {
            self.mark_dirty(view);
        }
        if commit.changes.intersects(PACED) {
            self.request_frame(commit.surface);
        }
    }

    fn destroyed(&mut self, surface: SurfaceId) {
        rikka_compositor_seat::surface_destroyed(self, surface);
        self.contents.forget(surface);
        self.ornament_destroyed(surface);
    }

    fn wants_refresh(&mut self, output: OutputId) {
        self.request_refresh(output);
    }
}

impl SeatHost for State {
    fn seats(&mut self) -> &mut Seats {
        &mut self.seats
    }

    fn display(&self) -> &DisplayHandle<Self> {
        &self.display
    }

    fn seat_event(&mut self, event: SeatEvent) {
        rikka_compositor_selection::seat_event(self, &event);
        self.seat_event_arrived(event);
    }
}

impl ShellHost for State {
    fn shell(&mut self) -> &mut Shell {
        &mut self.shell
    }

    fn shell_event(&mut self, event: ShellEvent) {
        self.shell_event_arrived(event);
    }

    fn admit_activation(&mut self, _request: &TokenRequest) -> bool {
        true
    }
}

impl SelectionHost for State {
    fn selections(&mut self) -> &mut Selections {
        &mut self.selections
    }

    fn trusts_selection(
        &mut self,
        _client: &Client,
        _seat: rikka_compositor_seat::SeatId,
        _kind: SelectionKind,
    ) -> bool {
        true
    }

    fn send_selection(&mut self, request: SendSelection) {
        self.send_session_selection(request);
    }

    fn send_drag(&mut self, request: SendDrag) {
        self.send_dropped(request);
    }

    fn selection_event(&mut self, event: SelectionEvent) {
        self.selection_event_arrived(event);
    }
}

impl OutputHost for State {
    fn outputs(&mut self) -> &mut Outputs {
        &mut self.outputs
    }

    fn output_bound(&mut self, _output: OutputId, resource: &WlOutput) {
        let client = resource.client();
        let surfaces: Vec<SurfaceId> = self
            .views
            .records
            .values()
            .flat_map(|record| record.placed.iter().map(|placement| placement.surface))
            .filter(|surface| {
                self.surfaces
                    .get(*surface)
                    .is_some_and(|node| node.client() == client)
            })
            .collect();
        for surface in surfaces {
            self.surfaces.enter(surface, resource);
        }
    }
}
