use rikka_compositor_seat::{SeatHost, SeatId};
use rikka_compositor_server::Client;
use rikka_compositor_shell::ShellHost;
use rikka_compositor_surface::SurfaceId;

use crate::event::{SelectionEvent, SendDrag, SendSelection};
use crate::kind::SelectionKind;
use crate::selections::Selections;

pub trait SelectionHost: SeatHost + ShellHost {
    fn selections(&mut self) -> &mut Selections;

    fn trusts_selection(&mut self, _client: &Client, _seat: SeatId, _kind: SelectionKind) -> bool {
        false
    }

    fn send_selection(&mut self, _request: SendSelection) {}

    fn send_drag(&mut self, _request: SendDrag) {}

    fn foreign_drag_target(&mut self, _surface: SurfaceId) -> bool {
        false
    }

    fn selection_event(&mut self, _event: SelectionEvent) {}
}
